use std::{collections::{HashMap, HashSet}, net::Ipv4Addr, sync::{Arc, Mutex}};

use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time;

pub mod api;

enum Event { RequestCompleted, }

struct Job {
    ip: Ipv4Addr,
    tx: oneshot::Sender<api::schema::Response>,
}

struct Client {
    event_broadcast: broadcast::Sender<Event>,
    job_queue: mpsc::Sender<Job>,
}

impl Client {
    pub fn new() -> Self {
        todo!()
    }
    pub fn request(ip: Ipv4Addr) -> oneshot::Receiver<api::schema::Response> {
        let (tx, rx) = oneshot::channel::<api::schema::Response>();



        rx
    }
}

mod actor {
    use std::collections::VecDeque;

    use super::*;

    /// Forwards any incoming jobs into the job queue.
    /// The input to this actor is external to the `actor` module,
    /// and should be used as the interface to issue requests.
    struct JobQueueAdapter {
        input: mpsc::Receiver<Job>,
        output: mpsc::Sender<JobQueueMessage>,
    }

    impl JobQueueAdapter {
        pub fn new(
            input: mpsc::Receiver<Job>, 
            output: mpsc::Sender<JobQueueMessage>
        ) -> Self {
            JobQueueAdapter { input, output }
        }
        pub async fn start(mut self) {
            while let Some(incoming_job) = self.input.recv().await {
                self.output.send(JobQueueMessage::Insert(incoming_job)).await.unwrap();
            }
        }
    }

    pub enum JobQueueMessage {
        Insert(Job),
        Remove(Ipv4Addr),
    }

    /// Manages the set of pending jobs. Denies new jobs that match
    /// an already-pending job.
    struct JobQueueManager {
        active_jobs: HashSet<Ipv4Addr>,
        input: mpsc::Receiver<JobQueueMessage>,
        output: mpsc::Sender<Job>,
    }

    impl JobQueueManager {
        pub fn new(
            input: mpsc::Receiver<JobQueueMessage>, 
            output: mpsc::Sender<Job>
        ) -> Self {
            let active_jobs = HashSet::new();
            Self { active_jobs, input, output }
        }
        pub async fn start(mut self) {
            while let Some(msg) = self.input.recv().await {
                self.handle_message(msg).await;
            }
        }
        async fn handle_message(&mut self, msg: JobQueueMessage) {
            match msg {
                JobQueueMessage::Insert(job) => {
                    if self.active_jobs.insert(job.ip) == true {
                        self.output.send(job).await.unwrap();
                    }
                }
                JobQueueMessage::Remove(ip) => {
                    self.active_jobs.remove(&ip);
                }
            }
        }
    }

    /// Dispatches jobs to either BATCH or SINGLE queries.
    /// TODO: - in accordance with rate limit
    struct Dispatcher {
        job_buffer: Vec<Job>,
        client: reqwest::Client,
        input: mpsc::Receiver<Job>,
        output: mpsc::Sender<(Job, api::schema::Response)>,
    }

    impl Dispatcher {
        pub fn new(
            input: mpsc::Receiver<Job>, 
            output: mpsc::Sender<(Job, api::schema::Response)>,
        ) -> Self {
            let job_buffer = Vec::new();
            let client = reqwest::Client::new();
            Self { job_buffer, client, input, output }
        }
        pub async fn start(mut self) {
            let timeout = time::Duration::from_secs(1);
            loop {
                tokio::select! {
                    Some(job) = self.input.recv() => {
                        self.job_buffer.push(job);
                    },
                    _ = time::sleep(timeout), if !self.job_buffer.is_empty() => {
                        self.dispatch_jobs();
                    }
                }
            }
        }
        fn dispatch_jobs(&mut self) {
            let jobs = self.job_buffer.drain(..).collect::<Vec<Job>>();
            let batches = jobs.into_iter().chunks(100);
            for batch in batches {

            }
            let remainder = batches.remainder() {

            }
            
        }
        fn spawn_batch(&self, jobs: &[Job]) {
            tokio::spawn({
                let ips = jobs.iter().map(|job| job.ip).collect::<Vec<Ipv4Addr>>();
                let client = self.client.clone();
                let output = self.output.clone();
                async move {
                    if let Ok(responses) = api::batch(&ips, client).await {
                        for (response, job) in responses.zip(jobs) {
                            assert_eq!(response.query, job.ip.to_string());
                            output.send((job, response)).await;
                        }
                    }
                }
            });
        }
    }
}

/*
struct Actors;

struct RequestHandler {
    job_queue: actors::JobQueue,
    dispatcher: actors::Dispatcher,
    joiner: actors::Joiner,
}

impl RequestHandler {
    fn new() -> Self {
        todo!()
    }

    fn start(self) -> mpsc::Sender<Ipv4Addr> {
        let pipeline_input = self.job_queue.handle.clone();
        let (tx, mut rx) = mpsc::channel(128);
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                pipeline_input.send(actors::JobQueueMessage::Insert { ip: msg }).await.unwrap();
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
*/