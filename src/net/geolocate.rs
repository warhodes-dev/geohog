use std::collections::{HashMap, HashSet};
use std::net::Ipv4Addr; 
use std::str::FromStr; 
use std::sync::{Arc, Mutex}; 
use std::time::Duration;
use actors::JobQueueMessage;
use hashlink::{LinkedHashMap, LinkedHashSet};
use tokio::sync::broadcast;
use tokio::sync::mpsc::{self, Receiver, Sender};
use ip_api::schema::IpApiResponse;

mod ip_api;

#[derive(Debug, Clone)]
pub enum Locator {
    Global(Geolocation),
    // TODO: Make use an enum for this
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

pub struct GeolocationClient {
    cache: Arc<Mutex<HashMap<Ipv4Addr, Locator>>>,
    request_channel: mpsc::Sender<Ipv4Addr>,
}

impl GeolocationClient {
    pub fn new() -> Self {
        let cache_inner = HashMap::new();
        let cache = Arc::new(Mutex::new(cache_inner));

        let request_handler = RequestHandler::new(cache.clone());
        let request_channel = request_handler.start();


        GeolocationClient { cache, request_channel }
    }

    pub fn geolocate(&mut self, ip: &Ipv4Addr) -> Option<Locator> {
        self.cache_get(ip)
            .or_else(|| {
                tokio::spawn({
                    let ip = *ip;
                    let tx = self.request_channel.clone();
                    async move { tx.send(ip).await }
                });
                None
            })
    }

    fn cache_get(&self, ip: &Ipv4Addr) -> Option<Locator> {
        self.cache
            .lock()
            .unwrap()
            .get(&ip)
            .cloned()
    }
}

struct RequestHandler {
    job_queue: actors::JobQueue,
    dispatcher: actors::Dispatcher,
    joiner: actors::Joiner,
}

impl RequestHandler {
    fn new(cache_ref: Arc<Mutex<HashMap<Ipv4Addr, Locator>>>) -> Self {
        let mut job_queue = actors::JobQueue::new();
        let mut dispatcher = actors::Dispatcher::new();
        let mut joiner = actors::Joiner::new(cache_ref);

        job_queue.outputs_to(dispatcher.handle.clone());
        dispatcher.outputs_to(joiner.handle.clone());
        joiner.outputs_to(job_queue.handle.clone());
        
        Self { job_queue, dispatcher, joiner }
    }

    fn start(self) -> mpsc::Sender<Ipv4Addr> {
        let pipeline_input = self.job_queue.handle.clone();
        let (tx, mut rx) = mpsc::channel(128);
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                pipeline_input.send(JobQueueMessage::Insert { ip: msg }).await.unwrap();
            }
        });
        tokio::spawn(self.job_queue.start());
        tokio::spawn(self.dispatcher.start());
        tokio::spawn(self.joiner.start());
        tx
    }
}

// * ============================================================

mod actors {
    use std::{collections::{HashMap, HashSet}, net::Ipv4Addr, str::FromStr, sync::{Arc, Mutex}};

    use tokio::sync::mpsc;

    use super::{ip_api, Locator};

    /// Initial receiver of requests. Filters out undesirable requests,
    /// such as duplicates.
    pub struct JobQueue {
        pub handle: mpsc::Sender<JobQueueMessage>,
        active_jobs: HashSet<Ipv4Addr>,
        input: mpsc::Receiver<JobQueueMessage>, // From: geolocate::request_geolocation()
        output: Option<mpsc::Sender<Ipv4Addr>>, // To:   actors::Dispatcher.input
    }

    pub enum JobQueueMessage {
        Insert { ip: Ipv4Addr },
        Remove { ip: Ipv4Addr },
    }

    impl JobQueue {
        pub fn new() -> Self {
            let (tx, rx) = mpsc::channel(128);
            JobQueue { 
                handle: tx,
                active_jobs: HashSet::new(),
                input: rx,
                output: None,
            }
        }
        pub fn outputs_to(&mut self, tx: mpsc::Sender<Ipv4Addr>) {
            self.output = Some(tx)
        }
        async fn handle_message(&mut self, msg: JobQueueMessage)  {
            let output = self.output.as_ref().unwrap();
            match msg {
                JobQueueMessage::Insert{ ip } => {
                    // Filter out requests that are already pending
                    if self.active_jobs.insert(ip) == true {
                        output.send(ip).await.unwrap();
                    }
                },
                JobQueueMessage::Remove{ ip } => {
                    self.active_jobs.remove(&ip);
                },
            }
        }
        pub async fn start(mut self) {
            assert!(self.output.is_some());
            while let Some(msg) = self.input.recv().await {
                self.handle_message(msg).await;
            }
        }
    }

    /// Dispatches a given request (or set of requests) as either a 
    /// BATCH or SINGLE query to Ip-Api.
    pub struct Dispatcher {
        pub handle: mpsc::Sender<Ipv4Addr>,
        client: reqwest::Client,
        input: mpsc::Receiver<Ipv4Addr>,        // From: JobQueue.output
        output: Option<mpsc::Sender<(Ipv4Addr, ip_api::schema::IpApiResponse)>> // To: Joiner.input
    }

    impl Dispatcher {
        pub fn new() -> Self {
            let (tx, rx) = mpsc::channel(128);
            let client = reqwest::Client::new();
            Dispatcher { 
                handle: tx,
                client,
                input: rx,
                output: None,
            }
        }
        pub fn outputs_to(&mut self, tx: mpsc::Sender<(Ipv4Addr, ip_api::schema::IpApiResponse)>) {
            self.output = Some(tx)
        }
        pub async fn start(mut self) {
            assert!(self.output.is_some());

            // TODO: Replace this with the actual rate limit
            let mut period = tokio::time::interval(std::time::Duration::from_secs(4));
            loop {
                period.tick().await;

                let mut jobs = Vec::new();
                let num_jobs = self.input.recv_many(&mut jobs, 100).await;
                if num_jobs > 3 {
                    self.batch(jobs.drain(..).as_slice());
                } else {
                    for job in jobs.drain(..) {
                        self.single(job);
                    }
                }
            }
        }
        fn batch(&self, ips: &[Ipv4Addr]) {
            tokio::spawn({
                let tx = self.output.as_ref().unwrap().clone();
                let client = self.client.clone();
                async move {
                    if let Ok(responses) = ip_api::batch(ips, client).await {
                        for response in responses {
                            let response_ip = Ipv4Addr::from_str(&response.query).unwrap();
                            tx.send((response_ip, response)).await.unwrap();
                        }
                    }
                }
            });
        }
        fn single(&self, ip: Ipv4Addr) {

        }
    }

    pub struct Joiner {
        pub handle: mpsc::Sender<(Ipv4Addr, ip_api::schema::IpApiResponse)>,
        cache: Arc<Mutex<HashMap<Ipv4Addr, Locator>>>,
        input: mpsc::Receiver<(Ipv4Addr, ip_api::schema::IpApiResponse)>,
        output: Option<mpsc::Sender<JobQueueMessage>>,
    }

    impl Joiner {
        pub fn new(cache: Arc<Mutex<HashMap<Ipv4Addr, Locator>>>) -> Self {
            let (tx, rx) = mpsc::channel(128);
            Joiner {
                handle: tx,
                cache,
                input: rx,
                output: None,
            }
        }
        pub fn outputs_to(&mut self, tx: mpsc::Sender<JobQueueMessage>) {
            self.output = Some(tx)
        }
        async fn handle_message(&mut self, (ip, response): (Ipv4Addr, ip_api::schema::IpApiResponse)) {
            let locator = Locator::from(response);
            self.cache.lock().unwrap().insert(ip, locator);
            self.output.as_ref().unwrap().send(JobQueueMessage::Remove{ip}).await.unwrap();
        }
        pub async fn start(mut self) {
            assert!(self.output.is_some());
            while let Some(msg) = self.input.recv().await {
                self.handle_message(msg).await;
            }
        }
    }
}

impl From<ip_api::schema::IpApiResponse> for Locator {
    fn from(value: ip_api::schema::IpApiResponse) -> Self {
        match value.variant {
            ip_api::schema::IpApiVariant::Success(res) => {
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
            ip_api::schema::IpApiVariant::Fail(res) => {
                Self::NonGlobal(res.message)
            },
        }
    }
}