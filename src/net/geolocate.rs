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

use crate::collections::QueueMap;

type SharedLocator = Arc<Mutex<Option<Locator>>>;

pub struct GeolocationClient {
    cache: Arc<Mutex<HashMap<Ipv4Addr, Locator>>>,
    task_queue: Arc<Mutex<QueueMap<Ipv4Addr, SharedLocator>>>,
}

impl GeolocationClient {
    pub fn new() -> Self {
        let cache = Arc::new(Mutex::new(HashMap::new()));
        let task_queue = Arc::new(Mutex::new(QueueMap::new()));

        RequestHandler::init(task_queue.clone(), cache.clone());

        GeolocationClient { cache, task_queue }
    }

    pub fn geolocate_ip(&mut self, 
        ip: &Ipv4Addr, 
    ) -> SharedLocator {
        if let Some(cached_locator) = self.cache_get(ip) {
            tracing::trace!("{ip:15}: Cache hit. Setting locator to cached value.");
            return Arc::new(Mutex::new(Some(cached_locator)));
        } else {
            tracing::trace!("{ip:15}: Cache miss. Checking task queue...");
            if let Some(in_progress_locator) = self.get_task(ip) {
                tracing::trace!("{ip:15}: Task already in queue. Returning in-progress SharedLocator.");
                return in_progress_locator;
            } else {
                tracing::trace!("{ip:15}: Task not found in queue. Enqueueing new task.");
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
        self.task_queue.lock().unwrap().push_back(*ip, locator.clone());
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
        task_queue: Arc<Mutex<QueueMap<Ipv4Addr, SharedLocator>>>,
        cache: Arc<Mutex<HashMap<Ipv4Addr, Locator>>>,
    ) {
        let (tx, rx) = mpsc::channel::<(Ipv4Addr, Locator)>(128);
        let pending_tasks_1 = Arc::new(Mutex::new(HashMap::new()));
        let pending_tasks_2 = pending_tasks_1.clone();
        tokio::spawn(async move {
            tracing::debug!("Spawning batching worker task.");
            RequestHandler::batcher_task(tx, pending_tasks_1, task_queue.clone()).await;

        });
        tokio::spawn(async move {
            tracing::debug!("Spawning joining worker task.");
            RequestHandler::joiner_task(rx, pending_tasks_2, cache.clone()).await;
        });
    }

    /// On interval, takes a batch of tasks and issues the appropriate API query 
    async fn batcher_task(
        response_tx: Sender<(Ipv4Addr, Locator)>,
        pending_tasks: Arc<Mutex<HashMap<Ipv4Addr, SharedLocator>>>,
        task_queue: Arc<Mutex<QueueMap<Ipv4Addr, SharedLocator>>>,
    ) {
        let mut interval = tokio::time::interval(Duration::from_secs(3));
        loop {
            interval.tick().await;
            tracing::trace!("Polling task queue...");
            {
                let mut task_queue = task_queue.lock().unwrap();
                let mut pending_tasks = pending_tasks.lock().unwrap();

                // TODO: Batch queries to avoid raid limit
                while let Some((ip, shared_locator)) = task_queue.pop_front() {
                    pending_tasks.insert(ip, shared_locator);
                    let tx = response_tx.clone();
                    tokio::spawn(async move {
                        if let Ok(response) = single_query(ip).await {
                            tx.send((ip, response)).await.unwrap();
                        }
                    });
                    tracing::trace!("{ip:15}: Task spawned: Inserted into pending queue.");
                }
            }
        }
    }

    /// Completes finished API queries by caching response and updating `SharedLocator`
    async fn joiner_task(
        mut response_rx: Receiver<(Ipv4Addr, Locator)>,
        pending_tasks: Arc<Mutex<HashMap<Ipv4Addr, SharedLocator>>>,
        cache: Arc<Mutex<HashMap<Ipv4Addr, Locator>>>,
    ) {
        while let Some((ip, locator)) = response_rx.recv().await {

            // Set shared locator to API result and remove task from pending set
            {
                let mut pending_tasks = pending_tasks.lock().unwrap();
                if let Some(shared_locator) = pending_tasks.get_mut(&ip) {
                    *shared_locator.lock().unwrap() = Some(locator.clone());
                } else {
                    tracing::error!("{ip:15}: Task completed, but not present in pending_tasks.")
                }
                pending_tasks.remove(&ip);
            }
            tracing::trace!("{ip:15}: Task joined. Inserted into cache and removed from pending queue.");
            cache.lock().unwrap().insert(ip, locator);
        }
    }
}

async fn single_query(ip: Ipv4Addr) -> Result<Locator> {
    tracing::info!("{ip:15}: Issuing single query to IpApi...");
    let service = Service::IpApi;

    //TODO: Drop the dependency, manual GET
    //TODO: Add config for multiple providers
    Locator::get_ipv4(ip, service)
        .await
        .map_err(anyhow::Error::msg)
}

async fn batch_query(ips: &[Ipv4Addr]) -> Result<Vec<Locator>> {
    tracing::info!("({:15}): Issuing BATCH query to IpApi...", format!("{} jobs", ips.len()));
    todo!();
}
