use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::sync::LazyLock;
use tokio::sync::RwLock;

use ipgeolocate::{Locator, Service};

use anyhow::Result;

//TODO: Refactor this into a client structure:
// struct GeoLocator {
//   cache: HashMap<ip, Locator>
// } impl GeoLocator {
//   fn geolocate_ip(ip) -> Result<Locator> {
//     ...
//   }
// }

pub struct GeolocationClient {
    cache: Arc<RwLock<HashMap<String, Locator>>>
}

impl GeolocationClient {
    pub fn new() -> Self {
        GeolocationClient {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /*
    pub fn geolocate_from_cache(&self, ip: &str) -> Option<Locator> {
        self.cache.blocking_read().get(ip).cloned()
    }
    */

    pub async fn geolocate_ip_from_cache(&self, ip: &str) -> Option<Locator> {
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
        tracing::info!("Querying IpApi for {ip}");
        let service = Service::IpApi;

        //TODO: Drop the dependency, manual GET 
        //TODO: Add config for multiple providers
        let response = Locator::get(ip, service).await?;
        Ok(response)
    }
}



/*
pub async fn geolocate_endpoints(
    endpoint_locations: Arc<Mutex<Vec<GeoLocation>>>,
) -> Result<(), String> {
    let service = Service::IpApi;
    let connections = net::get_tcp().map_err(|e| e.to_string())?;

    let mut new_endpoint_locations = Vec::new();

    for connection in connections {
        let geolocate_response = Locator::get(&connection.remote_address, service).await
            .ok()
            .map(|response| {
                GeoLocation {
                    ip: connection.remote_address.to_owned(),
                    lat: response.latitude.parse::<f64>().unwrap(),
                    long: response.longitude.parse::<f64>().unwrap(),
                }
            });

        if let Some(location) = geolocate_response {
            new_endpoint_locations.push(location);
        }
    }

    *endpoint_locations.lock().unwrap() = new_endpoint_locations;

    Ok(())
}

pub async fn geolocate_host(host_location: Arc<Mutex<Option<GeoLocation>>>) -> Result<()> {
    let service = Service::IpApi;

    let ip_raw = match public_ip::addr().await.unwrap() {
        IpAddr::V4(ipaddr) => ipaddr,
        IpAddr::V6(_) => { anyhow::bail!("IPV6 not supported") }
    };

    let ip = ip_raw.octets()
        .map(|byte| byte.to_string())
        .iter()
        .map(|s| s.as_ref())
        .intersperse(".")
        .collect::<String>();

    let geolocate = Locator::get(&ip, service).await
        .ok()
        .map(|response| {
            GeoLocation {
                ip: ip.to_owned(),
                lat: response.latitude.parse::<f64>().unwrap(),
                long: response.longitude.parse::<f64>().unwrap(),
            }
        });

    *host_location.lock().unwrap() = geolocate;
    Ok(())
}
*/