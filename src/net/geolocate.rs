use indexmap::IndexMap;
use std::{collections::HashMap, net::Ipv4Addr, sync::Arc, time::Duration};
use tokio::{
    sync::{
        mpsc::{self, Receiver, Sender},
        oneshot, Mutex,
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

    pub fn geolocate_ip(&mut self, ip: &Ipv4Addr, locator: Arc<Mutex<Option<Locator>>>) {
        let cached_locator = self.cache_get(ip);
        if cached_locator.is_some() {
            tracing::debug!("Cache hit for {ip}. Setting locator to cached value.");
            *locator.lock().unwrap() = cached_locator;
        } else {
            tracing::debug!("Cache miss for {ip}. Enqueuing task.");
            self.enqueue_task(ip, locator);
        }
    }

    fn cache_get(&self, ip: &Ipv4Addr) -> Option<Locator> {
        self.cache.lock().unwrap().get(&ip).cloned()
    }

    /// Enqueue task for RequestHandler without duplicates
    fn enqueue_task(&mut self, ip: &Ipv4Addr, locator: Arc<Mutex<Option<Locator>>>) {
        self.task_queue.blocking_lock().insert(*ip, locator.clone());
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
            task_queue,
            cache.clone(),
        ));
        tokio::spawn(RequestHandler::joiner_task(response_rx, cache.clone()));
    }

    /// On interval, takes a batch of tasks and issues the appropriate API query to be
    // handled later in `joiner_task`
    async fn batcher_task(
        response_tx: Sender<Locator>,
        task_queue: Arc<Mutex<IndexMap<Ipv4Addr, SharedLocator>>>,
        cache: Arc<Mutex<HashMap<Ipv4Addr, Locator>>>,
    ) {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            let mut task_queue_lock = task_queue.lock().unwrap();
            let tasks = task_queue_lock.iter().map(|pair| pair.1).cloned();
            drop(tasks);
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
