//! Request and subsequently access ip-geolocations from Ip-Api.com
//! 
//! [`request`] issues a request to Ip-Api.com and caches the result.
//! 
//! [`geolocate`] retrieves completed requests from the cache.
//! 
//! This enables the `ip_api::Client` to be easily used in synchronous code.
//! For example, in a user interface/dashboard, use [`request`] when a displayable
//! geolocatation object is constructed and [`geolocate`] during display.
//! 
//! Initially:      Connection: 12.34.56.78 -- Geolocation: Loading...
//! Refreshed:      Connection: 12.34.56.78 -- Geolocation: Leyndell, LB

use std::{
    collections::{HashMap, HashSet, VecDeque}, 
    net::Ipv4Addr, 
    str::FromStr, 
    sync::{Arc, Mutex}, time::Duration
};
use tokio::{sync::mpsc, time::Instant};
use api::schema::Response;

pub mod api;

/// A completed geolocation query.
#[derive(Debug, Clone)]
pub enum Locator {
    /// Ip address corresponds to a physical lat/long location.
    Global(Geolocation),
    /// Ip address does not correspond to any location,
    /// such as in reserved IP addresses e.g. 127.0.0.1.
    NonGlobal(String),
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

/// Public interface for requesting and retrieving IP geolocations.
pub struct Client {
    //event_broadcast: broadcast::Sender<Event>,
    /// Cached queries.
    cache: Arc<Mutex<HashMap<Ipv4Addr, Locator>>>,
    /// Tracks in-progress requests to prevent duplicate queries.
    active_requests: Arc<Mutex<HashSet<Ipv4Addr>>>,
    /// Handle to the RequestLimiter actor which batches and rate-limits queries.
    dispatch_queue: mpsc::UnboundedSender<Ipv4Addr>,
}

impl Client {
    pub fn new() -> Self {
        let cache_inner = HashMap::new();
        let cache = Arc::new(Mutex::new(cache_inner));

        let active_requests_inner = HashSet::new();
        let active_requests = Arc::new(Mutex::new(active_requests_inner));

        let dispatch_queue = RequestLimiter::spawn(
            cache.clone(),
            active_requests.clone(),
        );

        Self { cache, active_requests, dispatch_queue }
    }

    pub fn request(&mut self, ip: Ipv4Addr) {
        // Ignore requests that are already cached
        if self.cache.lock().unwrap().contains_key(&ip) {
            return;
        }

        if self.active_requests.lock().unwrap().insert(ip) {
            self.dispatch_queue.send(ip)
                .expect("Sending new job into RequestLimiter actor");
        }
    }

    pub fn geolocate(&self, ip: &Ipv4Addr) -> Option<Locator> {
        self.cache
            .lock()
            .unwrap()
            .get(&ip)
            .cloned()
    }
}

/// Minimum # of jobs that should be batched vs dispatched as single queries.
const BATCH_THRESHOLD: usize = 5;

// Amount of time to wait before dispatching all queued jobs.
const JOB_QUEUE_TIMEOUT: tokio::time::Duration = Duration::from_millis(1500);

struct RequestLimiter {
    /// Job accumulator for periodic batching/dispatching.
    job_buffer: VecDeque<Ipv4Addr>,

    /// Receiver for incoming requests.
    input: mpsc::UnboundedReceiver<Ipv4Addr>,

    /// HTTP session.
    http_client: reqwest::Client,

    /// Sender and Receiver for completed jobs to be: 
    ///     1. Added to cache.
    ///     2. Removed from active_requests.
    /// Handled by [`RequestLimiter::finish_job()`]
    finish_queue: FinishQueue,

    // Shared resources
    cache: Arc<Mutex<HashMap<Ipv4Addr, Locator>>>,
    active_requests: Arc<Mutex<HashSet<Ipv4Addr>>>,
}

struct FinishQueue {
    sender: mpsc::UnboundedSender<Response>,
    receiver: mpsc::UnboundedReceiver<Response>,
}

impl RequestLimiter {
    /// Initializes and *detaches* a task which will efficiently batch and 
    /// rate-limit requests to Ip-Api.com.
    fn spawn(
        cache: Arc<Mutex<HashMap<Ipv4Addr, Locator>>>,
        active_requests: Arc<Mutex<HashSet<Ipv4Addr>>>,
    ) -> mpsc::UnboundedSender<Ipv4Addr> {

        let http_client = reqwest::Client::new();
        let job_buffer = VecDeque::new();
        let (handle_tx, handle_rx) = mpsc::unbounded_channel();
        let (finish_tx, finish_rx) = mpsc::unbounded_channel();

        let finish_queue = FinishQueue {
            sender: finish_tx,
            receiver: finish_rx,
        };

        let request_limiter = Self { 
            input: handle_rx,
            finish_queue,
            job_buffer, 
            http_client,
            cache, 
            active_requests 
        };

        // Spawn and detach main RequestLimiter actor
        tokio::spawn(async { request_limiter.start().await });

        handle_tx
    }

    /// Main execution loop of RequestLimiter actor.
    async fn start(mut self) {
        tracing::debug!("Starting request limiter");
        let timeout = tokio::time::sleep(JOB_QUEUE_TIMEOUT);
        tokio::pin!(timeout);
        loop {
            tokio::select! {
                // Finish any outstanding tasks
                Some(job) = self.finish_queue.receiver.recv() => {
                    self.finish_job(job);
                }
                // Add incoming jobs to the job buffer.
                Some(job) = self.input.recv() => {
                    timeout.as_mut().reset(Instant::now() + JOB_QUEUE_TIMEOUT);
                    self.job_buffer.push_front(job);
                }
                // Dispatch all jobs in job_buffer
                () = &mut timeout => {
                    self.dispatch_jobs()
                }
            }
        }
    }

    /// Dispatches queued jobs in batches of 100 to the `/batch` endpoint.
    /// Also dispatches extranuous jobs (qty. < BATCH_THRESHOLD) to the `/` (single) endpoint.
    fn dispatch_jobs(&mut self) {
        use itertools::Itertools;
        let batches = self.job_buffer
            .drain(..) //FIXME: Only pull jobs that we can actually let through
            .into_iter()
            .chunks(100) // group jobs by batches of 100
            .into_iter()
            .map(|chunk| chunk.collect())
            .collect::<Vec<Vec<Ipv4Addr>>>();

        for batch in batches {
            if batch.len() > BATCH_THRESHOLD {
                let finish_queue = self.finish_queue.sender.clone();
                let client = self.http_client.clone();
                RequestLimiter::spawn_batch_task(batch, finish_queue, client);
            } else {
                for ip in batch {
                    let finish_queue = self.finish_queue.sender.clone();
                    let client = self.http_client.clone();
                    RequestLimiter::spawn_single_task(ip, finish_queue, client);
                }
            }
        }
    }

    fn spawn_batch_task(
        ips: Vec<Ipv4Addr>,
        finish_queue: mpsc::UnboundedSender<api::schema::Response>,
        client: reqwest::Client,
    ) {
        tokio::spawn(async move {
            if let Ok(responses) = api::batch(&ips, client).await {
                for response in responses {
                    finish_queue.send(response).unwrap();
                }
            }
        });
    }

    fn spawn_single_task(
        ip: Ipv4Addr,
        finish_queue: mpsc::UnboundedSender<api::schema::Response>,
        client: reqwest::Client,
    ) {
        tokio::spawn(async move {
            if let Ok(response) = api::single(&ip, client).await {
                finish_queue.send(response).unwrap();
            }
        });
    }

    /// Finishes a completed job by adding the response to the cache and
    /// removing the entry from active_jobs
    fn finish_job(&self, response: api::schema::Response) {
        let ip = Ipv4Addr::from_str(&response.query).unwrap();
        let locator = Locator::from(response);
        self.cache.lock().unwrap().insert(ip, locator);
        self.active_requests.lock().unwrap().remove(&ip);
    }
}

/* === Trait implementations === */

impl From<api::schema::Response> for Locator {
    fn from(value: api::schema::Response) -> Self {
        match value.status {
            api::schema::Status::Success(res) => {
                Self::Global(Geolocation {
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
            api::schema::Status::Fail(res) => {
                Self::NonGlobal(res.message)
            },
        }
    }
}