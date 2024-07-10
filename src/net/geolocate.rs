use std::{cell::RefCell, collections::{HashMap, VecDeque}, sync::{Arc, Mutex}};
use tokio::sync::{mpsc, RwLock};

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
    task_queue: VecDeque<GeolocationTask>,
    runtime: tokio::runtime::Handle,
}

impl GeolocationClient {
    pub fn new(runtime: tokio::runtime::Handle) -> Self {
        GeolocationClient {
            cache: Arc::new(RwLock::new(HashMap::new())),
            task_queue: VecDeque::new(),
            runtime
        }
    }

    fn _init_request_handler(&self) {
        let (tx, mut rx) = mpsc::channel::<GeolocationTask>(128);
        let request_handler = self.runtime.spawn(async move {

        });
    }

    pub fn cache_get(&self, ip: &str) -> Option<Locator> {
        self.cache.blocking_read().get(ip).cloned()
    }

    async fn async_cache_get(&self, ip: &str) -> Option<Locator> {
        self.cache.read().await.get(ip).cloned()
    }

    fn enqueue_task(&mut self, task: GeolocationTask) {
        self.task_queue.push_back(task);
    }

    pub fn geolocate_ips(&mut self, tasks: impl Iterator<Item = GeolocationTask>) -> Result<()> {
        let client = RefCell::new(self);

        // Complete any tasks that can be resolved from cache, filter these tasks out
        tasks.filter_map(|task| {
                let cached_locator = client.borrow().cache_get(&task.ip);
                if cached_locator.is_some() {
                    let mut loc = task.locator.lock().unwrap();
                    *loc = cached_locator;
                    None // job done, remove this task from queue
                } else {
                    Some(task) // further work required
                }
            })
            .for_each(|task| {
                client.borrow_mut().enqueue_task(task);
            });

        /* Batch large jobs to avoid rate limit
        if tasks.len() > 5 {
            // do batch
        } else {
            for task in task_queue {
                self.runtime.spawn({
                    let cache = Arc::clone(&self.cache);
                    let locator = Arc::clone(&task.locator);
                    async move {
                        if let Ok(new_locator) = geolocate_ip(&task.ip, &cache).await {
                            let mut locator_lock = locator.lock().unwrap();
                            *locator_lock = Some(new_locator);
                        }
                    }
                });
            }
        } */

        Ok(())
    }
}

async fn geolocate_ip(
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