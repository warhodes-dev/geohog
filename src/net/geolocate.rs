use std::collections::HashMap;
use std::net::Ipv4Addr; 
use std::str::FromStr; 
use std::sync::{Arc, Mutex}; 
use std::time::Duration;
use hashlink::LinkedHashMap;
use tokio::sync::mpsc::{self, Receiver, Sender};
use ip_api::schema::IpApiResponse;

mod ip_api;

pub type SharedLocator = Arc<Mutex<Locator>>;

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

/// Dispatches and joins Ip-Api request tasks
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

            let ips = job_queue.lock().unwrap()
                .iter_mut()
                .filter(|(_, job)| matches!(job.status, JobProgress::Queued))
                .take(100)
                .map(|(_, job)| {
                    job.status = JobProgress::Issued;
                    job.ip
                })
                .collect::<Vec<_>>();

            if ips.len() > 5 {

                // Batch query
                tokio::spawn({
                    let tx = response_tx.clone();
                    let client = client.clone();
                    async move {
                        if let Ok(responses) = ip_api::batch(ips.as_slice(), client).await {
                            for response in responses {
                                let response_ip = Ipv4Addr::from_str(&response.query).unwrap();
                                tx.send((response_ip, response)).await.unwrap();
                            }
                        }
                    }
                });

            } else {

                // Issue several single queries
                for ip in ips {
                    tokio::spawn({
                        let tx = response_tx.clone();
                        let client = client.clone();
                        async move {
                            if let Ok(response) = ip_api::single(ip, client).await {
                                let response_ip = Ipv4Addr::from_str(&response.query).unwrap();
                                tx.send((response_ip, response)).await.unwrap();
                            }
                        }
                    });
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
            let shared_locator = job_queue.lock().unwrap()
                .remove(&ip)
                .expect("Match completed job to entry in job queue")
                .locator;

            // update shared locator and store reference to it in the cache
            *shared_locator.lock().unwrap() = Locator::from(response);
            cache.lock().unwrap().insert(ip, shared_locator);
        }
    }
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