use std::{collections::{HashMap, HashSet}, net::Ipv4Addr, sync::{Arc, Mutex}};

use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time;

pub mod api;

enum Event { RequestCompleted, }

struct Job {
    ip: Ipv4Addr,
    tx: oneshot::Sender<api::schema::Response>,
}

pub struct Client {
    // event_broadcast: broadcast::Sender<Event>,
    requester_handle: mpsc::UnboundedSender<Job>,
}

impl Client {
    pub fn new() -> Self {
        let request_handler = RequestHandler::new();
        let requester_handle = request_handler.start();
        Self { requester_handle }
    }
    pub fn request(&self, ip: Ipv4Addr) -> oneshot::Receiver<api::schema::Response> {
        let (tx, rx) = oneshot::channel::<api::schema::Response>();
        let job = Job { ip, tx };
        self.requester_handle.send(job)
            .expect("Queueing job into the request handler.");
        rx
    }
}

struct RequestHandler {
    pub handle: mpsc::UnboundedSender<Job>,
    adapter: JobQueueAdapter,
    job_queue: JobQueueManager,
    dispatcher: Dispatcher,
    joiner: Joiner,
}

impl RequestHandler {
    fn new() -> Self {
        let (adapter_tx, adapter_rx) = mpsc::unbounded_channel();
        let (job_queue_tx, job_queue_rx) = mpsc::channel(1);
        let (dispatcher_tx, dispatcher_rx) = mpsc::channel(1);
        let (joiner_tx, joiner_rx) = mpsc::channel(64);

        // Don't cross your wires
        Self {
            handle: adapter_tx,
            adapter:   JobQueueAdapter::new(adapter_rx,    job_queue_tx.clone()),
            job_queue: JobQueueManager::new(job_queue_rx,  dispatcher_tx),
            dispatcher:     Dispatcher::new(dispatcher_rx, joiner_tx),
            joiner:             Joiner::new(joiner_rx,     job_queue_tx),
        }
    }
    fn start(self) -> mpsc::UnboundedSender<Job> {
        tokio::spawn(self.adapter.start());
        tokio::spawn(self.job_queue.start());
        tokio::spawn(self.dispatcher.start());
        tokio::spawn(self.joiner.start());
        self.handle
    }
}

/// Forwards any incoming jobs into the job queue.
/// The input to this actor is external to the `actor` module,
/// and should be used as the interface to issue requests.
struct JobQueueAdapter {
    input: mpsc::UnboundedReceiver<Job>,
    output: mpsc::Sender<JobQueueMessage>,
}

impl JobQueueAdapter {
    fn new(
        input: mpsc::UnboundedReceiver<Job>, 
        output: mpsc::Sender<JobQueueMessage>
    ) -> Self {
        JobQueueAdapter { input, output }
    }
        async fn start(mut self) {
        while let Some(incoming_job) = self.input.recv().await {
            self.output.send(JobQueueMessage::Insert(incoming_job)).await.unwrap();
        }
    }
}

enum JobQueueMessage {
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
    fn new(
        input: mpsc::Receiver<JobQueueMessage>, 
        output: mpsc::Sender<Job>
    ) -> Self {
        let active_jobs = HashSet::new();
        Self { active_jobs, input, output }
    }
    async fn start(mut self) {
        while let Some(msg) = self.input.recv().await {
            self.handle_message(msg).await;
        }
    }
    async fn handle_message(&mut self, msg: JobQueueMessage) {
        match msg {
            JobQueueMessage::Insert(job) => {
                if self.active_jobs.insert(job.ip) == true {
                    self.output.send(job).await
                        .expect("Sending job to Dispatcher");
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
    fn new(
        input: mpsc::Receiver<Job>, 
        output: mpsc::Sender<(Job, api::schema::Response)>,
    ) -> Self {
        let job_buffer = Vec::new();
        let client = reqwest::Client::new();
        Self { job_buffer, client, input, output }
    }
        async fn start(mut self) {
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
        use itertools::Itertools;
        let batches = self.job_buffer
            .drain(..)
            .into_iter()
            .chunks(100)
            .into_iter()
            .map(|chunk| chunk.collect())
            .collect::<Vec<Vec<Job>>>();
        
        for batch in batches {
            if batch.len() > 3 {
                self.spawn_batch(batch);
            } else {
                for job in batch {
                    self.spawn_single(job);
                }
            }
        }
    }
    fn spawn_batch(&self, jobs: Vec<Job>) {
        tokio::spawn({
            let ips = jobs.iter().map(|job| job.ip).collect::<Vec<Ipv4Addr>>();
            let client = self.client.clone();
            let output = self.output.clone();
            async move {
                if let Ok(responses) = api::batch(&ips, client).await {
                    for (response, job) in responses.zip(jobs) {
                        assert_eq!(response.query, job.ip.to_string());
                        output.send((job, response)).await
                            .expect("Sending completed job to Joiner");
                    }
                }
            }
        });
    }
    fn spawn_single(&self, job: Job) {
        tokio::spawn({
            let ip = job.ip;
            let client = self.client.clone();
            let output = self.output.clone();
            async move {
                if let Ok(response) = api::single(&ip, client).await {
                    assert_eq!(response.query, job.ip.to_string());
                    output.send((job, response)).await
                        .expect("Sending completed job to Joiner");
                }
            }
        });
    }
}

/// Joins completed API queries and returns them to the initial caller.
/// Also issues commands to the JobQueue for removal.
struct Joiner {
    input: mpsc::Receiver<(Job, api::schema::Response)>,
    output: mpsc::Sender<JobQueueMessage>,
}

impl Joiner {
        fn new(
        input: mpsc::Receiver<(Job, api::schema::Response)>,
        output: mpsc::Sender<JobQueueMessage>,
    ) -> Self {
        Self { input, output }
    }  
        async fn start(mut self) {
        while let Some((job, response)) = self.input.recv().await {
            job.tx.send(response)
                .expect("Returning Response to initial caller.");
            self.output.send(JobQueueMessage::Remove(job.ip)).await
                .expect("Issuing removal command to JobQueue");
        }
    }
}