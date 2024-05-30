use std::net::IpAddr;
use std::sync::{Arc, Mutex};

use ipgeolocate::{Locator, Service};

use anyhow::Result;

#[derive(Clone, Debug)]
pub struct GeoLocation {
    pub ip: String,
    pub lat: f64,
    pub long: f64,
    pub city: String,
    pub region: String,
    pub country: String,
    pub timezone: String,
    pub isp: String,
}

pub async fn geolocate_ip(ip: String) -> Result<Locator> {
    let service = Service::IpApi;

    //TODO: Drop the dependency, manual GET 
    //TODO: Add config for multiple providers
    let response = Locator::get(ip.as_str(), service).await?;
    Ok(response)
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