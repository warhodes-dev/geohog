use std::{collections::{HashMap, VecDeque}, sync::{Arc, Mutex}};
use tokio::sync::RwLock;

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

    pub fn from_cache(&self, ip: &str) -> Option<Locator> {
        self.cache.blocking_read().get(ip).cloned()
    }

    pub async fn async_from_cache(&self, ip: &str) -> Option<Locator> {
        self.cache_get(ip).await
    }

    pub fn queue_tasks(&mut self, tasks: impl Iterator<Item = GeolocationTask>) {
        for task in tasks {
            self.task_queue.push_back(task);
        }
    }

    pub fn geolocate_ips(&self, task_queue: impl Iterator<Item = GeolocationTask>) -> Result<()> {
        // Complete any tasks that can be resolved from cache, filter these tasks out
        let task_queue = task_queue.filter_map(|task| {
                let cached_locator = self.from_cache(&task.ip);
                if cached_locator.is_some() {
                    let mut loc = task.locator.lock().unwrap();
                    *loc = cached_locator;
                    None // job done, remove this task from queue
                } else {
                    Some(task) // further work required
                }
            })
            .collect::<Vec<GeolocationTask>>();

        // Batch large jobs to avoid rate limit
        if task_queue.len() > 5 {
            for task_batch in task_queue.chunks(100) {

            }
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
        }

        Ok(())
    }



    async fn cache_get(&self, ip: &str) -> Option<Locator> {
        self.cache.read().await.get(ip).cloned()
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