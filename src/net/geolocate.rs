use std::collections::{HashMap, HashSet};
use std::net::Ipv4Addr; 
use std::str::FromStr; 
use std::sync::{Arc, Mutex}; 
use std::time::Duration;
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
    //request_handler: todo!(),
}

impl GeolocationClient {
    pub fn new() -> Self {
        let cache_inner = HashMap::new();
        let cache = Arc::new(Mutex::new(cache_inner));

        GeolocationClient { cache }
    }

    pub fn geolocate(&mut self, ip: &Ipv4Addr) -> Option<Locator> {
        self.cache_get(ip)
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
    
}

impl RequestHandler {
    fn new() -> Self {
        todo!()
    }
}



// * ============================================================

#[derive(Message)]
#[rtype(result = "()")]
enum JobQueueMessage {
    Insert(Ipv4Addr),
    Remove(Ipv4Addr),
}

use actix::prelude::*;

struct JobQueue {
    active_jobs: HashSet<Ipv4Addr>,
}

impl JobQueue {
    fn new() -> Self {
        JobQueue { job_queue: HashSet::new() }
    }
}

impl Actor for JobQueue {
    type Context = Context<Self>;
}

impl Handler<JobQueueMessage> for JobQueue {
    type Result = ();

    fn handle(&mut self, msg: JobQueueMessage, _ctx: &mut Context<Self>) {
        match msg {
            JobQueueMessage::Insert(item) => {
                if self.active_jobs.insert(item) {
                    //send item --> dispatcher
                }
            },
            JobQueueMessage::Remove(item) => { self.active_jobs.remove(&item); },
        }
    }
}

// * ============================================================

    /*
    pub struct RequestHandler {
        job_queue: JobQueueHandle
    }

    impl RequestHandler {
        pub fn new() -> Self {  
            todo!()
        }
        pub fn start() -> Self {
            todo!()
        }
    }




    struct JobQueue {
        active_jobs: HashSet<Ipv4Addr>,
        // Input: job queue management commands 
        input: mpsc::Receiver<JobQueueMessage>,
        // Output: jobs to be dispatched
        output: DispatchHandle,
    }

    impl JobQueue {
        fn new(dispatcher: DispatchHandle) -> JobQueueHandle {
            let active_jobs = HashSet::new();
            let (tx, rx) = mpsc::channel(128);
            let actor = JobQueue { 
                active_jobs, 
                input: rx, 
                output: dispatcher 
            };
            tokio::spawn(actor.start());
            JobQueueHandle { tx }
        }
        fn handle_message(&mut self, msg: JobQueueMessage) {
            match msg {
                JobQueueMessage::Insert(ip) => {
                    // Filter out requests that are already pending
                    if self.active_jobs.insert(ip) == true {
                        self.output.send(ip);
                    }
                },
                JobQueueMessage::Remove(ip) => {
                    self.active_jobs.remove(&ip);
                },
            }
        }
        async fn start(mut self) {
            while let Some(msg) = self.input.recv().await {
                self.handle_message(msg)
            }
        }
    }

    struct JobQueueHandle {
        tx: mpsc::Sender<JobQueueMessage>
    }

    impl JobQueueHandle {
        pub fn insert(&self, ip: Ipv4Addr) {
            self.send(JobQueueMessage::Insert(ip));
        }
        pub fn remove(&self, ip: Ipv4Addr) {
            self.send(JobQueueMessage::Remove(ip));
        }
        fn send(&self, cmd: JobQueueMessage) {
            tokio::spawn({
                let tx = self.tx.clone();
                async move { tx.send(cmd).await }
            });
        }
    }

    struct Dispatch {
        // Input: Jobs to be dispatched
        input: mpsc::Receiver<Ipv4Addr>,

        // Forwarded output: Completed jobs to be joined
        // This handle is forwarded to API query tasks.
        output: JoinerHandle,
    }

    impl Dispatch {
        fn new(joiner: JoinerHandle) -> DispatchHandle {
            let (tx, rx) = mpsc::channel(128);
            let actor = Dispatch { 
                input: rx, 
                output: joiner
            };
            tokio::spawn(actor.start());
            DispatchHandle { tx }
        }
        fn handle_message(&mut self, msg: Ipv4Addr) {
            msg;
            todo!()
        }
        async fn start(mut self) {
            while let Some(msg) = self.input.recv().await {
                self.handle_message(msg)
            }
        }
    }

    struct DispatchHandle {
        tx: mpsc::Sender<Ipv4Addr>
    }

    impl DispatchHandle {
        fn send(&self, ip: Ipv4Addr) {
            tokio::spawn({
                let tx = self.tx.clone();
                async move { tx.send(ip).await }
            });
        }
    }

    struct Joiner {
        cache: Arc<Mutex<HashMap<Ipv4Addr, Locator>>>,
        // Input: Completed API queries (jobs)
        input: mpsc::Receiver<ip_api::schema::IpApiResponse>,
        // Output: Job queue management commands (removal)
        output: JobQueueHandle,
    }

    impl Joiner {
        fn new(
            cache: Arc<Mutex<HashMap<Ipv4Addr, Locator>>>,
            rx: mpsc::Receiver<ip_api::schema::IpApiResponse>,
            job_queue: JobQueueHandle,
        ) -> JoinerHandle {
            let (tx, rx) = mpsc::channel(128);
            let actor = Joiner {
                cache,
                input: rx,
                output: job_queue,
            };
            tokio::spawn(actor.start());
            JoinerHandle { tx }
        }
        fn handle_message(&mut self, msg: ip_api::schema::IpApiResponse) {
            let ip = Ipv4Addr::from_str(&msg.query).expect("Converting JSON IP to Ipv4Addr");
            let locator = Locator::from(msg);
            self.cache.lock().unwrap().insert(ip, locator);

            self.output.remove(ip);
        }
        async fn start(mut self) {
            while let Some(msg) = self.input.recv().await {
                self.handle_message(msg)
            }
        }
    }

    struct JoinerHandle {
        tx: mpsc::Sender<ip_api::schema::IpApiResponse>
    }

    impl JoinerHandle {
        fn send(&self, ip: ip_api::schema::IpApiResponse) {
            tokio::spawn({
                let tx = self.tx.clone();
                async move { tx.send(ip).await }
            });
        }
    }
}

struct RequestHandler2;
impl RequestHandler2 {
    /// Initialize and spawn all request handler actors. Essentially, this actor system 
    /// asynchronously updates the provided cache with the results of completed queries
    /// Returns a request queue sender that can be used to enqueue jobs.
    fn init(cache: Arc<Mutex<HashMap<Ipv4Addr, Locator>>>) -> mpsc::Sender<JobQueueCommand> {

        // Incoming jobs that need to be processed
        let (job_queue_tx, job_queue_rx) = mpsc::channel::<JobQueueCommand>(1024);

        // Filtered jobs that need to be dispatched
        let (dispatch_tx, dispatch_rx) = mpsc::channel::<Ipv4Addr>(1024);

        // Completed jobs that need to be joined, cached, and removed from job queue
        let (join_tx, join_rx) = mpsc::channel::<(Ipv4Addr, ip_api::schema::IpApiResponse)>(1024);

        tokio::spawn(RequestHandler2::request_listener(job_queue_rx, dispatch_tx));
        tokio::spawn(RequestHandler2::dispatcher(job_queue_ptr.clone(), job_join_tx));
        tokio::spawn(RequestHandler2::joiner(cache.clone(), job_queue_tx.clone(), job_join_rx));
        job_queue_tx
    }

    /// Populates the job queue from receiving message channel.
    async fn request_listener(
        mut rx: mpsc::Receiver<JobQueueCommand>,
        tx: mpsc::Sender<Ipv4Addr>,
    ) {
        tracing::debug!("Spawning 'job queue' actor.");
        let mut active_jobs = HashSet::<Ipv4Addr>::new();
        while let Some(command) = rx.recv().await {
            
        }
    }

    /// Dispatches API requests from job queue.
    async fn dispatcher(
        job_queue: Arc<Mutex<LinkedHashMap<Ipv4Addr, Job>>>, 
        mut tx: mpsc::Sender<(Ipv4Addr, ip_api::schema::IpApiResponse)>
    ) {
        tracing::debug!("Spawning 'dispatcher' actor.");
        let client = reqwest::Client::new();
        let mut interval = tokio::time::interval(Duration::from_secs(3));

        loop { 
            interval.tick().await; 

            let ips = job_queue.lock().unwrap()
                .iter_mut()
                .filter(|(_, job)| matches!(job.status, JobStatus::Queued))
                .take(100)
                .map(|(ip, job)| {
                    job.issue();
                    *ip
                })
                .collect::<Vec<_>>();

            if ips.len() > 5 {

                // Batch query
                tokio::spawn({
                    let tx = tx.clone();
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
                        let tx = tx.clone();
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

    /// Updates the cache as request are completed.
    async fn joiner(
        cache: Arc<Mutex<HashMap<Ipv4Addr, Locator>>>,
        tx: mpsc::Sender<JobQueueCommand>,
        mut rx: mpsc::Receiver<(Ipv4Addr, ip_api::schema::IpApiResponse)>,
    ) {
        tracing::debug!("Spawning 'joiner' actor.");
        while let Some((ip, response)) = rx.recv().await {

            // Store locator in the cache
            let locator = Locator::from(response);
            cache.lock().unwrap().insert(ip, locator);

            // Send command to remove ip from job queue.
            tokio::spawn({
                let tx = tx.clone();
                async move { tx.send(JobQueueCommand::Remove(ip)).await }
            });
        }
    }
}
    */


struct Job {
    status: JobStatus,
}

enum JobStatus {
    Queued,
    Issued,
}

impl Job {
    fn new() -> Self {
        Job{ status: JobStatus::Queued }
    }

    fn issue(&mut self) {
        self.status = JobStatus::Issued;
    }
}

// ====== Trait implementations =======================


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