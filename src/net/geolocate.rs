use std::{collections::{HashMap, HashSet}, net::Ipv4Addr, sync::{Arc, Mutex}, time::Duration};
use serde::Deserialize;
use tokio::{
    sync::{
        mpsc::{self, Receiver, Sender},
        oneshot,
    },
    task::JoinHandle,
};
use hashlink::LinkedHashMap;
use anyhow::{Result, anyhow, bail};

type SharedLocator = Arc<Mutex<Option<Locator>>>;

pub struct GeolocationClient {
    cache: Arc<Mutex<HashMap<Ipv4Addr, CachedLocator>>>,
    blacklist: Arc<Mutex<HashSet<Ipv4Addr>>>,
    job_queue: Arc<Mutex<LinkedHashMap<Ipv4Addr, Job>>>,
}

impl GeolocationClient {
    pub fn new() -> Self {
        let cache = Arc::new(Mutex::new(HashMap::new()));
        let job_queue = Arc::new(Mutex::new(LinkedHashMap::new()));
        let blacklist = Arc::new(Mutex::new(HashSet::new()));

        RequestHandler::init(job_queue.clone(), cache.clone());

        GeolocationClient { cache, job_queue, blacklist }
    }

    pub fn geolocate_ip(&mut self, ip: &Ipv4Addr) -> SharedLocator {
        if let Some(cached_locator) = self.cache_get(ip) {
            match cached_locator {
                CachedLocator::Valid(locator) => {
                    tracing::trace!("{ip:15}: Cache hit. Setting locator to cached value.");
                    Arc::new(Mutex::new(Some(locator)))
                },
                CachedLocator::Refused(reason) => {
                    tracing::debug!("{ip:15}: Cache hit, but API recently refused this IP for {reason}. Returning doomed locator.");
                    Arc::new(Mutex::new(None))
                }
            }
        } else {
            tracing::trace!("{ip:15}: Cache miss. Checking job queue...");
            self.enqueue_job(ip)
        }
    }

    fn cache_get(&self, ip: &Ipv4Addr) -> Option<CachedLocator> {
        self.cache.lock().unwrap().get(&ip).cloned()
    }

    /// Enqueue job for RequestHandler without duplicates
    fn enqueue_job(&mut self, ip: &Ipv4Addr) -> SharedLocator {
        let mut job_queue = self.job_queue.lock().unwrap();
        if let Some(in_progress_job) = job_queue.get(ip) {
            tracing::trace!("Refusing to enqueue duplicate job. Returning in-progress locator.");
            let in_progress_locator = in_progress_job.locator.clone();
            return in_progress_locator;
        } else {
            tracing::trace!("Enqueueing job. Returning new locator.");
            let new_locator = Arc::new(Mutex::new(None));
            let job = Job::new(*ip, new_locator.clone());
            job_queue.insert(job.ip, job);
            return new_locator;
        }
    }
}

#[derive(Debug, Clone)]
pub struct Locator {
    pub ip: String,
    pub latitude: f64,
    pub longitude: f64,
    pub continent: String,
    pub continent_code: String,
    pub country: String,
    pub country_code: String,
    pub region: String,
    pub region_code: String,
    pub city: String,
    pub timezone: String,
    pub isp: String,
}

impl Locator {
    fn from_ip_api(res: IpApiSuccess) -> Self {
        Locator {
            ip:             res.query,
            latitude:       res.lat,
            longitude:      res.lon,
            continent:      res.continent,
            continent_code: res.continentCode,
            country:        res.country,
            country_code:   res.countryCode,
            region:         res.regionName,
            region_code:    res.region,
            city:           res.city,
            timezone:       res.timezone,
            isp:            res.isp,
        }
    }
}

#[derive(Clone)]
enum CachedLocator {
    Valid(Locator),
    Refused(IpApiError),
}

impl TryFrom<Result<Locator>> for CachedLocator {
    type Error = anyhow::Error;
    
    fn try_from(value: Result<Locator>) -> std::result::Result<Self, Self::Error> {
        match value {
            Ok(locator) => Ok(CachedLocator::Valid(locator)),
            Err(err) => {
                let ip_api_error = err.downcast::<IpApiError>()?;
                Ok(CachedLocator::Refused(ip_api_error))
            }
        }
    }
}

#[derive(Clone)]
struct Job {
    pub ip: Ipv4Addr,
    pub locator: SharedLocator,
    pending: bool,
}

impl Job {
    fn new(ip: Ipv4Addr, locator: SharedLocator) -> Self {
        Job { ip, locator, pending: false }
    }
}

/// Handles Ip-Api requests
struct RequestHandler;
impl RequestHandler {
    fn init(
        job_queue: Arc<Mutex<LinkedHashMap<Ipv4Addr, Job>>>,
        cache: Arc<Mutex<HashMap<Ipv4Addr, CachedLocator>>>,
    ) {
        let (tx, rx) = mpsc::channel::<(Ipv4Addr, Result<Locator>)>(128);

        tokio::spawn({ // Batching Task
            let job_queue = job_queue.clone();
            async move { RequestHandler::batching_worker(tx, job_queue).await; }
        });

        tokio::spawn({ // Joiner Task
            let job_queue = job_queue.clone();
            let cache = cache.clone();
            async move { RequestHandler::joining_worker(rx, job_queue, cache).await; }
        });
    }

    /// On interval, takes a batch of tasks and issues the appropriate API query 
    async fn batching_worker(
        response_tx: Sender<(Ipv4Addr, Result<Locator>)>,
        job_queue: Arc<Mutex<LinkedHashMap<Ipv4Addr, Job>>>,
    ) {
        tracing::debug!("Spawning 'batch' worker task.");
        let mut interval = tokio::time::interval(Duration::from_secs(3));
        loop {
            tracing::trace!("Polling task queue...");
            interval.tick().await;
            {
                let mut job_queue = job_queue.lock().unwrap();

                // TODO: Batch queries to avoid raid limit
                for (&ip, job) in job_queue.iter_mut() {
                    tracing::trace!("{ip:15}: Spawning job: Inserting into pending queue.");
                    job.pending = true;
                    tokio::spawn({
                        let tx = response_tx.clone();
                        async move {
                            let response = single_query(ip).await;
                            tx.send((ip, response)).await.unwrap();
                        }
                    });
                }
            }
        }
    }

    /// Completes finished API queries by caching response and updating `SharedLocator`
    async fn joining_worker(
        mut response_rx: Receiver<(Ipv4Addr, Result<Locator>)>,
        job_queue: Arc<Mutex<LinkedHashMap<Ipv4Addr, Job>>>,
        cache: Arc<Mutex<HashMap<Ipv4Addr, CachedLocator>>>,
    ) {
        tracing::debug!("Spawning 'join' worker task.");
        while let Some((ip, response)) = response_rx.recv().await {

            // Set shared locator to API result and remove pending job from job queue
            tracing::trace!("{ip:15} Job complete. Removing job from pending queue.");
            let job = job_queue.lock().unwrap()
                .remove(&ip)
                .expect("Job joined but not found in task queue.");

            // Set shared locator which presumably exists and is in-use elsewhere.
            // TODO: Set this to some kind of trinary state like Pending/Done(val)/Refused(why)
            if let Ok(ref locator) = response {
                *job.locator.lock().unwrap() = Some(locator.clone());
            }

            // Only cache Ok or Err(IpApiError). Other errors (e.g. reqwest)
            // should be fine to re-attempt later. 
            if let Ok(cacheable_locator) = CachedLocator::try_from(response) {
                cache.lock().unwrap().insert(ip, cacheable_locator);
            } else {
                tracing::warn!("Request failure for {ip}. Ignoring.");
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum IpApiResponse {
    Success(IpApiSuccess),
    Fail(IpApiFail),
}

#[derive(Deserialize)]
struct IpApiFail {
    status: String,
    message: String,
    query: String,
}

#[allow(non_snake_case)]
#[derive(Deserialize)]
struct IpApiSuccess {
    continent: String,
    continentCode: String,
    country: String,
    countryCode: String,
    region: String,
    regionName: String,
    city: String,
    lat: f64,
    lon: f64,
    timezone: String,
    isp: String,
    query: String,
}

#[derive(Debug, Clone)]
enum IpApiError {
    PrivateRange,
    ReservedRange,
    InvalidQuery,
    Unknown,
}

impl<T: AsRef<str>> From<T> for IpApiError {
    fn from(value: T) -> Self {
        match value.as_ref() {
            "private range" => IpApiError::PrivateRange,
            "reserved range" => IpApiError::ReservedRange,
            "invalid query" => IpApiError::InvalidQuery,
            _ => IpApiError::Unknown,
        }
    }
}

impl std::fmt::Display for IpApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            IpApiError::PrivateRange => "private range",
            IpApiError::ReservedRange => "reserved range",
            IpApiError::InvalidQuery => "invalid query",
            IpApiError::Unknown => "unknown server error",
        };
        write!(f, "{msg}")
    }
}

async fn single_query(ip: Ipv4Addr) -> Result<Locator> {
    tracing::info!("{ip:15}: Issuing single query to IpApi...");
    let url = format!("http://ip-api.com/json/{ip}?fields=status,message,continent,\
                       continentCode,country,countryCode,region,regionName,city,\
                       lat,lon,timezone,isp,query");

    let response = reqwest::get(&url).await
        .inspect_err(|err| tracing::error!("ip-api error: {err}"))?
        .json::<IpApiResponse>()
        .await
        .inspect_err(|err| tracing::error!("serde json parse error: {err:?}"))?;


    match response {
        IpApiResponse::Success(success) => Ok(Locator::from_ip_api(success)),
        IpApiResponse::Fail(fail) => bail!(IpApiError::from(&fail.message)),
    }
}

async fn batch_query(ips: &[Ipv4Addr]) -> Result<Vec<Locator>> {
    tracing::info!("({:15}): Issuing BATCH query to IpApi...", format!("{} jobs", ips.len()));
    todo!();
}
