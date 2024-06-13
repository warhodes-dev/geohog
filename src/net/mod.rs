use std::{collections::HashMap, sync::{Arc, Mutex}};
use procfs::process::{FDTarget, Stat};

use ipgeolocate::Locator;

use anyhow::Result;

use geolocate::GeolocationClient;

pub mod geolocate;
pub mod public_ip;

pub struct Connection {
    pub local_address: String,
    pub local_address_port: String,
    pub remote_address: String,
    pub remote_address_port: String,
    pub state: String,
    pub inode: u64,
    pub pid: i32,
    pub comm: String,
    pub geolocation: Arc<Mutex<Option<Locator>>>
}

impl Connection {
    pub fn display(&self) -> ConnectionDisplay {
        todo!()
    }
}

pub struct ConnectionDisplay { /* TODO: fill this out */ }

pub struct NetClient {
    connections: Vec<Connection>,
    geolocation_client: Arc<GeolocationClient>,
    runtime: tokio::runtime::Handle,
}

impl NetClient {
    pub fn new(runtime: &tokio::runtime::Runtime) -> Self {
        let connections = vec![];
        let geolocation_client = Arc::new(GeolocationClient::new());
        let handle = runtime.handle().clone();
        NetClient {
            connections,
            geolocation_client,
            runtime: handle,
        }
    }

    pub fn connections<'a>(&'a self) -> impl Iterator<Item = &'a Connection> {
        self.connections.iter()
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.get_proc_connections()?;
        self.restore_geolocations();

        Ok(())
    }

    pub fn geolocate_connections(&self) {
        for con in self.connections.iter() {
            let remote_address = con.remote_address.clone();
            let current_geo = &con.geolocation.lock().unwrap();
            if current_geo.is_none() {
                self.runtime.spawn({
                    let geo = Arc::clone(&con.geolocation);
                    let client = Arc::clone(&self.geolocation_client);
                    async move {
                        if let Ok(new_geo) = client.geolocate_ip(&remote_address).await {
                            let mut geo_lock = geo.lock().unwrap();
                            *geo_lock = Some(new_geo);
                        }
                    }
                });
            }
        }
    }

    fn restore_geolocations(&mut self) {
        for con in self.connections.iter_mut() {
            let remote_address = &con.remote_address;
            let mut geo_lock = con.geolocation.lock().unwrap();
            *geo_lock = self.geolocation_client.from_cache(remote_address);
        }
    }

    fn get_proc_connections(&mut self) -> Result<()> {
        tracing::debug!("Getting TCP connections");

        let all_procs = procfs::process::all_processes().unwrap();

        let mut proc_map: HashMap<u64, Stat> = HashMap::new();
        for process_result in all_procs {
            if let Ok(process) = process_result 
            && let (Ok(stat), Ok(fdt)) = (process.stat(), process.fd()) {
                for fd_result in fdt {
                    if let Ok(fd) = fd_result
                    && let FDTarget::Socket(inode) = fd.target {
                        proc_map.insert(inode, stat.clone());
                    }
                }
            }
        }

        let tcp = procfs::net::tcp().unwrap();

        self.connections.clear();

        for entry in tcp.iter() {
            let local_address_string = entry.local_address.to_string();
            let (local_address, local_address_port) = local_address_string.split_once(':').unwrap();

            let remote_address_string = entry.remote_address.to_string();
            let (remote_address, remote_address_port) = remote_address_string.split_once(':').unwrap();

            //TODO: Actually format this
            let state = format!("{:?}", entry.state);
            if let Some(stat) = proc_map.get(&entry.inode) {
                let connection = Connection {
                    local_address: local_address.to_owned(),
                    local_address_port: local_address_port.to_owned(),
                    remote_address: remote_address.to_owned(),
                    remote_address_port: remote_address_port.to_owned(),
                    state,
                    inode: entry.inode,
                    pid: stat.pid,
                    comm: stat.comm.clone(),
                    geolocation: Arc::new(Mutex::new(None))
                };

                self.connections.push(connection);
            }
        }
        Ok(())
    }
}


