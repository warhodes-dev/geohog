use std::{collections::HashMap, io, time::{Duration, Instant}};
use procfs::process::{FDTarget, Stat};

use anyhow::Result;

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
}

pub fn get_tcp() -> Result<Vec<Connection>> {
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

    let mut connections = Vec::new();

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
            };

            connections.push(connection);
        }
    }

    Ok(connections)
}