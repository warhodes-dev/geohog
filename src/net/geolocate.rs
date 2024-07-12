use std::{borrow::Borrow, cell::RefCell, collections::{HashMap, HashSet}, net::Ipv4Addr, sync::{mpsc::Receiver, Arc, Mutex}};
use tokio::{sync::{mpsc, oneshot, RwLock}, task::JoinHandle};

use ipgeolocate::{Locator, Service};

use anyhow::Result;

pub struct GeolocationClient {
    cache: Arc<RwLock<HashMap<Ipv4Addr, Locator>>>,
    task_tx: mpsc::Sender<(Ipv4Addr, oneshot::Sender<()>)>,
    pending_requests: Arc<Mutex<HashSet<Ipv4Addr>>>,
    runtime: tokio::runtime::Handle,
}

impl GeolocationClient {
    pub fn new(runtime: tokio::runtime::Handle) -> Self {
        let cache = Arc::new(RwLock::new(HashMap::new()));
        let (task_tx, task_rx) = mpsc::channel::<(Ipv4Addr, oneshot::Sender<()>)>(1024);
        let pending_requests = Arc::new(Mutex::new(HashSet::new()));

        RequestHandler::init(runtime.clone(), task_rx, cache.clone());

        GeolocationClient {
            cache,
            pending_requests,
            task_tx,
            runtime,
        }
    }

    pub fn geolocate_ip(&mut self, ip: &Ipv4Addr) -> Option<Locator> {
        let geolocation = self.cache_get(ip);
        if geolocation.is_none() {
            tracing::debug!("Cache miss for {ip}. Enqueuing task.");
            self.enqueue_task(ip);
        } else {
            tracing::debug!("Cache hit for {ip}. Returning");
        }
        geolocation
    }

    fn cache_get(&self, ip: &Ipv4Addr) -> Option<Locator> {
        self.cache.blocking_read().get(&ip).cloned()
    }

    /// Enqueue task for RequestHandler without duplicates
    fn enqueue_task(&self, task: &Ipv4Addr) {
        let is_request_pending = self.pending_requests.lock().unwrap().contains(task);
        if !is_request_pending {
            tracing::debug!("Enqueued task for {task}. Adding {task} to pending_queue.");
            self.pending_requests.lock().unwrap().insert(task.to_owned());

            // Spawn task to 1. enqueue task 2. maintain pending_requests
            let pending_requests = Arc::clone(&self.pending_requests);
            let task_tx = self.task_tx.to_owned();
            let task = task.to_owned();
            self.runtime.spawn(async move {
                let (response_tx, response_rx) = oneshot::channel();
                let msg = (task.clone(), response_tx);

                // Enqueue task on mpsc channel for later processing
                task_tx.send(msg).await.unwrap();

                // Upon completion, remove the task from the pending set
                response_rx.await.unwrap();
                tracing::debug!("Request response recieved. Removing {task} from pending_queue.");
                pending_requests.lock().unwrap().remove(&task);
            });
        } else {
            tracing::debug!("Task {task} already in queue. Ignoring duplicate request.")
        }
    }
}

/// Handles Ip-Api requests
struct RequestHandler;
impl RequestHandler {
    fn init(
        runtime: tokio::runtime::Handle,
        receiver: mpsc::Receiver<(Ipv4Addr, oneshot::Sender<()>)>, 
        cache: Arc<RwLock<HashMap<Ipv4Addr, Locator>>>,
    ) -> JoinHandle<()> {
        runtime.spawn(RequestHandler::worker_task(receiver, cache))
    }

    async fn worker_task(
        mut receiver: mpsc::Receiver<(Ipv4Addr, oneshot::Sender<()>)>, 
        cache: Arc<RwLock<HashMap<Ipv4Addr, Locator>>>,
    ) {
        while let Some((ip, response_tx)) = receiver.recv().await {
            tracing::debug!("Request receieved. Handling");
            let cache_ptr = cache.clone();
            tokio::spawn(async move {
                geolocate_and_cache_ip(ip, cache_ptr).await;
                response_tx.send(()).unwrap();
            });
        }
    }
}

async fn geolocate_and_cache_ip(
    ip: Ipv4Addr, 
    cache: Arc<RwLock<HashMap<Ipv4Addr, Locator>>>
) {
    if let Ok(locator) = single_query(&ip).await {
        cache.write().await.insert(ip.to_owned(), locator.to_owned());
        tracing::debug!("Locator for {ip} cached successfully");
    }
}
    
async fn single_query(ip: &Ipv4Addr) -> Result<Locator> {
    tracing::info!("Querying IpApi for {ip}");
    let service = Service::IpApi;

    //TODO: Drop the dependency, manual GET 
    //TODO: Add config for multiple providers
    Locator::get_ipv4(*ip, service).await.map_err(anyhow::Error::msg)
}

async fn batch_query(ip: &str) -> Result<Vec<Locator>> {
    todo!();
}