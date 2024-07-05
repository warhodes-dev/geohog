use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use ipgeolocate::{Locator, Service};

use anyhow::Result;

pub struct GeolocationClient {
    cache: Arc<RwLock<HashMap<String, Locator>>>
}

impl GeolocationClient {
    pub fn new() -> Self {
        GeolocationClient {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn from_cache(&self, ip: &str) -> Option<Locator> {
        self.cache.blocking_read().get(ip).cloned()
    }

    pub async fn async_from_cache(&self, ip: &str) -> Option<Locator> {
        self.cache_get(ip).await
    }

    pub async fn geolocate_ip(&self, ip: &str) -> Result<Locator> {
        let locator = self.cache_get(ip).await;
        if locator.is_some() {
            Ok(locator.unwrap())
        } else {
            let locator = self.geolocate_query(ip).await?;
            self.cache.write().await.insert(ip.to_owned(), locator.to_owned());
            Ok(locator)
        }
    }

    async fn cache_get(&self, ip: &str) -> Option<Locator> {
        self.cache.read().await.get(ip).cloned()
    }
    
    async fn geolocate_query(&self, ip: &str) -> Result<Locator> {
        tracing::debug!("Querying IpApi for {ip}");
        let service = Service::IpApi;

        //TODO: Drop the dependency, manual GET 
        //TODO: Add config for multiple providers
        let response = Locator::get(ip, service).await?;
        Ok(response)
    }
}