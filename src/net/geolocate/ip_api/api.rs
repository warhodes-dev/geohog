//! REST API endpoints for Ip-Api (www.ip-api.com)
use std::net::Ipv4Addr;
use anyhow::Result;

const DEFAULT_FIELDS: &str = "status,message,continent,continentCode,\
                            country,countryCode,region,regionName,city,\
                            lat,lon,timezone,isp,query";

/// Queries a single IP address, returning a single geolocation.
/// Rate limit: 45/min
pub async fn single(ip: Ipv4Addr, client: reqwest::Client) -> Result<schema::Response> {
    tracing::info!("{ip:15}: Issuing SINGLE query to IpApi...");
    let url = format!("http://ip-api.com/json/{ip}?fields={DEFAULT_FIELDS}");

    let response = client.get(&url)
        .send().await?
        .json::<schema::Response>().await?;

    Ok(response)
}

/// Queries a batch of up to 100 IP addresses, returning an array of geolocations in order.
/// Rate limit: 15/min
pub async fn batch(ips: &[Ipv4Addr], client: reqwest::Client) -> Result<impl Iterator<Item = schema::Response>> {
    tracing::info!("({:13}): Issuing BATCH query to IpApi...", format!("{} jobs", ips.len()));
    let url = format!("http://ip-api.com/batch?fields={DEFAULT_FIELDS}");

    let response = client.post(&url)
        .json(&ips)
        .send().await?
        .json::<Vec<schema::Response>>().await?;

    Ok(response.into_iter())
}

pub mod schema {
    use serde::Deserialize;

    #[derive(Deserialize)]
    pub struct Response {
        pub query: String,
        #[serde(flatten)]
        pub status: Status,
    }

    #[derive(Deserialize)]
    #[serde(tag = "status", rename_all = "lowercase")]
    pub enum Status {
        Success(Location),
        Fail(Error),
    }

    #[derive(Deserialize)]
    pub struct Error {
        pub message: String,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Location {
        pub continent: String,
        pub continent_code: String,
        pub country: String,
        pub country_code: String,
        pub region: String,
        pub region_name: String,
        pub city: String,
        pub lat: f64,
        pub lon: f64,
        pub timezone: String,
        pub isp: String,
    }
}