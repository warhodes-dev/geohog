use std::{cell::RefCell, collections::HashMap, sync::{mpsc::Receiver, Arc, Mutex}};
use tokio::{sync::{mpsc, RwLock}, task::JoinHandle};

use ipgeolocate::{Locator, Service};

use anyhow::Result;

pub struct GeolocationTask {
    ip: String,
    locator: Arc<Mutex<Option<Locator>>>,
}

impl GeolocationTask {
    pub fn new(ip: String, locator: Arc<Mutex<Option<Locator>>>) -> Self {
        GeolocationTask { ip, locator }
    }
}

pub struct GeolocationClient {
    cache: Arc<RwLock<HashMap<String, Locator>>>,
    task_queue: mpsc::Sender<String>,
    runtime: tokio::runtime::Handle,
}

impl GeolocationClient {
    pub fn new(runtime: tokio::runtime::Handle) -> Self {

        let cache = Arc::new(RwLock::new(HashMap::new()));

        let (sender, receiver) = mpsc::channel::<String>(1024);

        RequestHandler::init(
            runtime.clone(), 
            receiver,
            Arc::clone(&cache),
        );

        GeolocationClient {
            cache,
            task_queue: sender,
            runtime,
        }
    }

    pub fn geolocate_ip(&mut self, ip: &str) -> Option<Locator> {
        let geolocation = self.cache_get(ip);
        if geolocation.is_none() {
            self.enqueue_task(ip);
        }
        geolocation
    }

    fn cache_get(&self, ip: &str) -> Option<Locator> {
        self.cache.blocking_read().get(ip).cloned()
    }

    fn enqueue_task(&self, task: &str) {
        let task_queue = self.task_queue.clone();
        let task = task.to_owned();
        self.runtime.spawn(async move {
            task_queue.send(task).await.expect("Task queue no longer exists");
        });
    }
}

/// Handles 
struct RequestHandler;
impl RequestHandler {
    fn init(
        runtime: tokio::runtime::Handle,
        receiver: mpsc::Receiver<String>, 
        cache: Arc<RwLock<HashMap<String, Locator>>>,
    ) -> JoinHandle<Result<()>> {
        runtime.spawn(RequestHandler::worker_task(receiver, cache))
    }

    async fn worker_task(
        mut receiver: mpsc::Receiver<String>,
        cache: Arc<RwLock<HashMap<String, Locator>>>,
    ) -> Result<()> {
        while let Some(ip) = receiver.recv().await {
            let _ = geolocate_and_cache_ip(&ip, &cache).await?;
        }
        Ok(())
    }
}

async fn geolocate_and_cache_ip(
    ip: &str, 
    cache: &Arc<RwLock<HashMap<String, Locator>>>
) -> Result<Locator> {
    let locator = single_query(ip).await?;
    cache.write().await.insert(ip.to_owned(), locator.to_owned());
    Ok(locator)
}
    
async fn single_query(ip: &str) -> Result<Locator> {
    tracing::debug!("Querying IpApi for {ip}");
    let service = Service::IpApi;

    //TODO: Drop the dependency, manual GET 
    //TODO: Add config for multiple providers
    let response = Locator::get(ip, service).await?;
    Ok(response)
}

async fn batch_query(ip: &str) -> Result<Vec<Locator>> {
    todo!();
}