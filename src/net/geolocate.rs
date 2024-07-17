use indexmap::IndexMap;
use std::{collections::HashMap, net::Ipv4Addr, sync::Arc, sync::Mutex, time::Duration};
use tokio::{
    sync::{
        mpsc::{self, Receiver, Sender},
        oneshot,
    },
    task::JoinHandle,
};

use ipgeolocate::{Locator, Service};

use anyhow::Result;

type SharedLocator = Arc<std::sync::Mutex<Option<Locator>>>;

pub struct GeolocationClient {
    cache: Arc<Mutex<HashMap<Ipv4Addr, Locator>>>,
    task_queue: Arc<Mutex<IndexMap<Ipv4Addr, SharedLocator>>>,
}

impl GeolocationClient {
    pub fn new() -> Self {
        let cache = Arc::new(Mutex::new(HashMap::new()));
        let task_queue = Arc::new(Mutex::new(IndexMap::new()));

        RequestHandler::init(task_queue.clone(), cache.clone());

        GeolocationClient { cache, task_queue }
    }

    pub fn geolocate_ip(&mut self, 
        ip: &Ipv4Addr, 
    ) -> SharedLocator {
        if let Some(cached_locator) = self.cache_get(ip) {
            tracing::trace!("Cache hit for {ip}. Setting locator to cached value.");
            Arc::new(Mutex::new(Some(cached_locator)))
        } else {
            tracing::trace!("Cache miss for {ip}. Checking task queue...");
            if let Some(in_progress_locator) = self.get_task(ip) {
                tracing::trace!("Task already in queue. in-progress SharedLocator.");
                return in_progress_locator;
            } else {
                tracing::trace!("Task not found in queue. Enqueueing new task.");
                let empty_locator = Arc::new(Mutex::new(None));
                self.enqueue_task(ip, empty_locator.clone());
                return empty_locator;
            }
        }
    }

    fn cache_get(&self, ip: &Ipv4Addr) -> Option<Locator> {
        self.cache.lock().unwrap().get(&ip).cloned()
    }

    /// Enqueue task for RequestHandler without duplicates
    fn enqueue_task(&mut self, ip: &Ipv4Addr, locator: SharedLocator) {
        self.task_queue.lock().unwrap().insert(*ip, locator.clone());
    }

    /// Gets an existing task currently in the queue
    fn get_task(&self, ip: &Ipv4Addr) -> Option<SharedLocator> {
        self.task_queue.lock().unwrap().get(ip).cloned()
    }
}

/// Handles Ip-Api requests
struct RequestHandler;
impl RequestHandler {
    fn init(
        task_queue: Arc<Mutex<IndexMap<Ipv4Addr, SharedLocator>>>,
        cache: Arc<Mutex<HashMap<Ipv4Addr, Locator>>>,
    ) {
        let (response_tx, response_rx) = mpsc::channel::<Locator>(128);
        tokio::spawn(RequestHandler::batcher_task(
            response_tx,
            task_queue.clone(),
        ));
        tokio::spawn(RequestHandler::joiner_task(response_rx, cache.clone()));
    }

    /// On interval, takes a batch of tasks and issues the appropriate API query to be
    // handled later in `joiner_task`
    async fn batcher_task(
        response_tx: Sender<Locator>,
        task_queue: Arc<Mutex<IndexMap<Ipv4Addr, SharedLocator>>>,
    ) {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            let mut task_queue_lock = task_queue.lock().unwrap();
            drop(task_queue_lock);
            interval.tick().await;
        }
    }

    /// Completes finished API queries by caching response and updating `SharedLocator`
    async fn joiner_task(
        response_rx: Receiver<Locator>,
        cache: Arc<Mutex<HashMap<Ipv4Addr, Locator>>>,
    ) {
        todo!();
    }
}

async fn single_query(ip: &Ipv4Addr) -> Result<Locator> {
    tracing::info!("Querying IpApi for {ip}");
    let service = Service::IpApi;

    //TODO: Drop the dependency, manual GET
    //TODO: Add config for multiple providers
    Locator::get_ipv4(*ip, service)
        .await
        .map_err(anyhow::Error::msg)
}

async fn batch_query(ip: &str) -> Result<Vec<Locator>> {
    todo!();
}
