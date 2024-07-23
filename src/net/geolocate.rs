use std::{borrow::Borrow, collections::{HashMap, HashSet}, net::Ipv4Addr, ops::Deref, str::FromStr, sync::{Arc, Mutex}, time::Duration};
use ip_api::schema::IpApiResponse;
use tokio::sync::mpsc::{self, Receiver, Sender};
use hashlink::LinkedHashMap;
use anyhow::Result;

pub type SharedLocator = Arc<Mutex<Locator>>;

pub struct GeolocationClient {
    cache: Arc<Mutex<HashMap<Ipv4Addr, SharedLocator>>>,
    job_queue: Arc<Mutex<LinkedHashMap<Ipv4Addr, Job>>>,
}

impl GeolocationClient {
    pub fn new() -> Self {
        let cache = Arc::new(Mutex::new(HashMap::new()));
        let job_queue = Arc::new(Mutex::new(LinkedHashMap::new()));

        RequestHandler::init(job_queue.clone(), cache.clone());

        GeolocationClient { cache, job_queue}
    }

    pub fn geolocate_ip(&mut self, ip: &Ipv4Addr) -> SharedLocator {
        self.cache_get(ip)
            .unwrap_or_else(|| self.enqueue_job(ip))
    }

    fn cache_get(&self, ip: &Ipv4Addr) -> Option<SharedLocator> {
        self.cache
            .lock()
            .unwrap()
            .get(&ip)
            .cloned()
    }

    /// Enqueue job for RequestHandler without duplicates
    fn enqueue_job(&mut self, ip: &Ipv4Addr) -> SharedLocator {
        self.job_queue
            .lock()
            .unwrap()
            .entry(*ip)
            .or_insert(Job::new(*ip))
            .locator
            .clone()
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
        match value.variant {
            ip_api::schema::IpApiVariant::Success(res) => {
                Self::Some(Geolocation {
                    ip:             value.query,
                    latitude:       res.lat,
                    longitude:      res.lon,
                    continent:      res.continent,
                    continent_code: res.continent_code,
                    country:        res.country,
                    country_code:   res.country_code,
                    region:         res.region_name,
                    region_code:    res.region,
                    city:           res.city,
                    timezone:       res.timezone,
                    isp:            res.isp,
                })
            },
            ip_api::schema::IpApiVariant::Fail(res) => {
                Self::Refused(res.message)
            },
        }
    }
}

#[derive(Clone)]
struct Job {
    ip: Ipv4Addr,
    locator: SharedLocator,
    status: JobProgress,
}

#[derive(Clone)]
enum JobProgress {
    Queued,
    Issued,
}

impl Job {
    fn new(ip: Ipv4Addr) -> Self {
        let locator = Arc::new(Mutex::new(Locator::Pending));
        Job { ip, locator, status: JobProgress::Queued }
    }
}

/// Handles Ip-Api requests
struct RequestHandler;
impl RequestHandler {
    fn init(
        job_queue: Arc<Mutex<LinkedHashMap<Ipv4Addr, Job>>>,
        cache: Arc<Mutex<HashMap<Ipv4Addr, SharedLocator>>>,
    ) {
        let (tx, rx) = mpsc::channel::<(Ipv4Addr, ip_api::schema::IpApiResponse)>(1024);

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
        response_tx: Sender<(Ipv4Addr, ip_api::schema::IpApiResponse)>,
        job_queue: Arc<Mutex<LinkedHashMap<Ipv4Addr, Job>>>,
    ) {
        tracing::debug!("Spawning 'batch' worker task.");

        let client = reqwest::Client::new();
        let mut interval = tokio::time::interval(Duration::from_secs(3));

        loop {
            interval.tick().await;
            tracing::trace!("Polling task queue...");
            {
                let jobs = job_queue.lock().unwrap()
                    .iter_mut()
                    .filter(|(_, job)| matches!(job.status, JobProgress::Queued))
                    .take(100)
                    .map(|(ip, job)| {
                        job.status = JobProgress::Issued;
                        *ip
                    })
                    .collect::<Vec<_>>();

                if jobs.len() > 5 {
                    // Batch query
                    tracing::trace!("{} job(s): Spawning batch task.", jobs.len());
                    tokio::spawn({
                        let tx = response_tx.clone();
                        let client = client.clone();
                        async move {
                            let job_ips = jobs.as_slice();
                            match ip_api::batch_query(job_ips, client).await {
                                Ok(responses) => {
                                    let responses = job_ips.iter().zip(responses);
                                    for (&job_ip, response) in responses {
                                        let response_ip = Ipv4Addr::from_str(&response.query).unwrap();
                                        assert_eq!(job_ip, response_ip);
                                        tx.send((response_ip, response)).await.unwrap();
                                    }
                                },
                                Err(e) => tracing::error!("Query error:\n{e:?}")
                            }
                        }
                    });
                } else {
                    // Issue several single queries
                    for job in jobs {
                        let job_ip = job;
                        tracing::trace!("{job_ip:15}: Spawning single task.");
                        tokio::spawn({
                            let tx = response_tx.clone();
                            let client = client.clone();
                            async move {
                                match ip_api::single_query(job_ip, client).await {
                                    Ok(response) => {
                                        let response_ip = Ipv4Addr::from_str(&response.query).unwrap();
                                        assert_eq!(job_ip, response_ip);
                                        tx.send((response_ip, response)).await.unwrap();
                                    },
                                    Err(e) => tracing::error!("Query error:\n{e:?}")
                                }
                            }
                        });
                    }
                }
            }
        }
    }

    /// Completes finished API queries by caching response and updating `SharedLocator`
    async fn joining_worker(
        mut response_rx: Receiver<(Ipv4Addr, IpApiResponse)>,
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

            tracing::trace!("{ip:15}: Setting shared locator");
            *shared_locator.lock().unwrap() = Locator::from(response);

            tracing::trace!("{ip:15}: Adding shared locator to cache");
            cache.lock().unwrap().insert(ip, shared_locator);
        }
    }
}

mod ip_api {
    use std::net::Ipv4Addr;
    use anyhow::Result;

    pub mod schema {
        use serde::Deserialize;

        #[derive(Deserialize)]
        pub struct IpApiResponse {
            pub query: String,
            #[serde(flatten)]
            pub variant: IpApiVariant,
        }

        #[derive(Deserialize)]
        #[serde(tag = "status", rename_all = "lowercase")]
        pub enum IpApiVariant {
            Success(IpApiSuccess),
            Fail(IpApiFail),
        }

        #[derive(Deserialize)]
        pub struct IpApiFail {
            pub message: String,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct IpApiSuccess {
            pub continent: String,
            pub continent_code: String,
            pub country: String,
            pub country_code: String,
            pub region: String,
            pub region_name: String,
            pub city: String,
            pub lat: f64,
            pub lon: f64,
            pub timezone: String,
            pub isp: String,
        }
    }
    
    const DEFAULT_FIELDS: &str = "status,message,continent,continentCode,\
                                country,countryCode,region,regionName,city,\
                                lat,lon,timezone,isp,query";

    pub async fn single_query(ip: Ipv4Addr, client: reqwest::Client) -> Result<schema::IpApiResponse> {
        tracing::info!("{ip:15}: Issuing SINGLE query to IpApi...");
        let url = format!("http://ip-api.com/json/{ip}?fields={DEFAULT_FIELDS}");

        let response = client.get(&url)
            .send().await?
            .json::<schema::IpApiResponse>().await?;

        Ok(response)
    }

    pub async fn batch_query(ips: &[Ipv4Addr], client: reqwest::Client) -> Result<impl Iterator<Item = schema::IpApiResponse>> {
        tracing::info!("({:15}): Issuing BATCH query to IpApi...", format!("{} jobs", ips.len()));
        let url = format!("http://ip-api.com/batch?fields={DEFAULT_FIELDS}");

        let response = client.post(&url)
            .json(&ips)
            .send().await?
            .json::<Vec<schema::IpApiResponse>>().await?;

        Ok(response.into_iter())
    }
}