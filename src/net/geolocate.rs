use std::{borrow::Borrow, collections::{HashMap, HashSet}, net::Ipv4Addr, ops::Deref, sync::{Arc, Mutex}, time::Duration};
use ip_api::schema::IpApiResponse;
use tokio::sync::mpsc::{self, Receiver, Sender};
use hashlink::LinkedHashMap;
use anyhow::Result;

pub type SharedLocator = Arc<Mutex<Locator>>;

pub struct GeolocationClient {
    cache: Arc<Mutex<HashMap<Ipv4Addr, SharedLocator>>>,
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
            match cached_locator.lock().unwrap().deref() {
                Locator::Some(_) => tracing::trace!("{ip:15}: Cache hit"),
                Locator::Refused(reason) => tracing::debug!("{ip:15}: Cache hit, but API recently refused this IP for {reason}"),
                Locator::Pending => tracing::warn!("{ip:15}: Pending job is cached, which sould never happen!"),
            };
            cached_locator
        } else {
            tracing::trace!("{ip:15}: Cache miss. Checking job queue...");
            self.enqueue_job(ip)
        }
    }

    fn cache_get(&self, ip: &Ipv4Addr) -> Option<SharedLocator> {
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
            let new_locator = Arc::new(Mutex::new(Locator::Pending));
            let job = Job::new(*ip, new_locator.clone());
            job_queue.insert(job.ip, job);
            return new_locator;
        }
    }
}

#[derive(Debug, Clone)]
pub enum Locator {
    Some(Geolocation),
    Refused(String),
    Pending,
}

#[derive(Debug, Clone)]
pub struct Geolocation {
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

impl From<ip_api::schema::IpApiResponse> for Locator {
    fn from(value: ip_api::schema::IpApiResponse) -> Self {
        match value {
            ip_api::schema::IpApiResponse::Success(res) => {
                Self::Some(Geolocation {
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
                })
            },
            ip_api::schema::IpApiResponse::Fail(res) => {
                Self::Refused(res.message)
            },
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
        cache: Arc<Mutex<HashMap<Ipv4Addr, SharedLocator>>>,
    ) {
        let (tx, rx) = mpsc::channel::<(Ipv4Addr, Result<ip_api::schema::IpApiResponse>)>(128);

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
        response_tx: Sender<(Ipv4Addr, Result<ip_api::schema::IpApiResponse>)>,
        job_queue: Arc<Mutex<LinkedHashMap<Ipv4Addr, Job>>>,
    ) {
        tracing::debug!("Spawning 'batch' worker task.");
        let mut interval = tokio::time::interval(Duration::from_secs(3));
        loop {
            interval.tick().await;
            tracing::trace!("Polling task queue...");
            {
                let mut job_queue = job_queue.lock().unwrap();

                // TODO: Batch queries to avoid raid limit
                for (&ip, job) in job_queue.iter_mut() {
                    tracing::trace!("{ip:15}: Spawning job: Inserting into pending queue.");
                    job.pending = true;
                    tokio::spawn({
                        let tx = response_tx.clone();
                        async move {
                            let response = ip_api::single_query(ip).await;
                            tx.send((ip, response)).await.unwrap();
                        }
                    });
                }
            }
        }
    }

    /// Completes finished API queries by caching response and updating `SharedLocator`
    async fn joining_worker(
        mut response_rx: Receiver<(Ipv4Addr, Result<IpApiResponse>)>,
        job_queue: Arc<Mutex<LinkedHashMap<Ipv4Addr, Job>>>,
        cache: Arc<Mutex<HashMap<Ipv4Addr, SharedLocator>>>,
    ) {
        tracing::debug!("Spawning 'join' worker task.");
        while let Some((ip, response)) = response_rx.recv().await {

            // Remove pending job from job queue
            tracing::trace!("{ip:15}: Removing job from pending queue.");
            let shared_locator = job_queue.lock().unwrap()
                .remove(&ip)
                .expect("Job joined but not found in task queue.")
                .locator;

            if let Ok(response) = response {
                tracing::trace!("{ip:15}: Setting shared locator");
                *shared_locator.lock().unwrap() = Locator::from(response);

                tracing::trace!("{ip:15}: Adding shared locator to cache");
                cache.lock().unwrap().insert(ip, shared_locator);
            } else {
                response.inspect_err(|e| tracing::error!("Ip-Api request error:\n{e:?}")).ok();
            }
        }
    }
}

mod ip_api {
    use std::net::Ipv4Addr;
    use anyhow::Result;

    pub mod schema {
        use serde::Deserialize;
        #[derive(Deserialize)]
        #[serde(tag = "status", rename_all = "lowercase")]
        pub enum IpApiResponse {
            Success(IpApiSuccess),
            Fail(IpApiFail),
        }

        #[derive(Deserialize)]
        pub struct IpApiFail {
            pub message: String,
            pub query: String,
        }

        #[allow(non_snake_case)]
        #[derive(Deserialize)]
        pub struct IpApiSuccess {
            pub continent: String,
            pub continentCode: String,
            pub country: String,
            pub countryCode: String,
            pub region: String,
            pub regionName: String,
            pub city: String,
            pub lat: f64,
            pub lon: f64,
            pub timezone: String,
            pub isp: String,
            pub query: String,
        }
    }
    
    const DEFAULT_FIELDS: &str = "status,message,continent,continentCode,\
                                country,countryCode,region,regionName,city,\
                                lat,lon,timezone,isp,query";

    pub async fn single_query(ip: Ipv4Addr) -> Result<schema::IpApiResponse> {
        tracing::info!("{ip:15}: Issuing single query to IpApi...");
        let url = format!("http://ip-api.com/json/{ip}?fields={DEFAULT_FIELDS}");

        let response_text = reqwest::get(&url).await?
            .text().await?;

        let response = serde_json::from_str(&response_text)
            .map_err(|e| anyhow::anyhow!(format!("{e:?}\n\nResponse text:\n{response_text}")))?; 
        Ok(response)
    }

    /*
        async fn batch_query(ips: &[Ipv4Addr]) -> Result<Vec<Locator>> {
            tracing::info!("({:15}): Issuing BATCH query to IpApi...", format!("{} jobs", ips.len()));
            let url = format!("http://ip-api.com/batch/{ip}?fields={DEFAULT_FIELDS}");
        }
    */

}