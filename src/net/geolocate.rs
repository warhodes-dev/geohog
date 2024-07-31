use std::collections::{HashMap, HashSet};
use std::net::Ipv4Addr; 
use std::str::FromStr; 
use std::sync::{Arc, Mutex}; 
use std::time::Duration;
use hashlink::{LinkedHashMap, LinkedHashSet};
use ip_api::api;
use tokio::sync::broadcast;
use tokio::sync::mpsc::{self, Receiver, Sender};

mod ip_api;

#[derive(Debug, Clone)]
pub enum Locator {
    Global(Geolocation),
    // TODO: Make use an enum for this
    NonGlobal(String),
}

#[derive(Debug, Clone)]
pub struct Geolocation {
    pub ip: String,
    pub latitude: f64,
    pub longitude: f64,
    pub continent: String,
    pub continent_code: String,
    pub country: String,
    pub country_code: String,
    pub region: String,
    pub region_code: String,
    pub city: String,
    pub timezone: String,
    pub isp: String,
}

pub struct GeolocationClient {
    cache: Arc<Mutex<HashMap<Ipv4Addr, Locator>>>,
    request_channel: mpsc::Sender<Ipv4Addr>,
}

impl GeolocationClient {
    pub fn new() -> Self {
        let cache_inner = HashMap::new();
        let cache = Arc::new(Mutex::new(cache_inner));

        let (request_channel, rx) = mpsc::channel(128);
        panic!("unfinished. remove this shit");

        GeolocationClient { cache, request_channel }
    }

    pub fn geolocate(&mut self, ip: &Ipv4Addr) -> Option<Locator> {
        self.cache_get(ip)
            .or_else(|| {
                tokio::spawn({
                    let ip = *ip;
                    let tx = self.request_channel.clone();
                    async move { tx.send(ip).await }
                });
                None
            })
    }

    fn cache_get(&self, ip: &Ipv4Addr) -> Option<Locator> {
        self.cache
            .lock()
            .unwrap()
            .get(&ip)
            .cloned()
    }
}

impl From<api::schema::Response> for Locator {
    fn from(value: api::schema::Response) -> Self {
        match value.status {
            api::schema::Status::Success(res) => {
                Self::Global(Geolocation {
                    ip:             value.query,
                    latitude:       res.lat,
                    longitude:      res.lon,
                    continent:      res.continent,
                    continent_code: res.continent_code,
                    country:        res.country,
                    country_code:   res.country_code,
                    region:         res.region_name,
                    region_code:    res.region,
                    city:           res.city,
                    timezone:       res.timezone,
                    isp:            res.isp,
                })
            },
            api::schema::Status::Fail(res) => {
                Self::NonGlobal(res.message)
            },
        }
    }
}