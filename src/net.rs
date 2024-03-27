use std::{collections::HashMap, io, time::{Duration, Instant}};
use procfs::process::{FDTarget, Stat};

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

pub fn get_tcp() -> Result<Vec<Connection>, Box<dyn std::error::Error>> {

    let all_procs = procfs::process::all_processes().unwrap();

    let mut map: HashMap<u64, Stat> = HashMap::new();

    for process_result in all_procs {
        if let Ok(process) = process_result 
        && let (Ok(stat), Ok(fdt)) = (process.stat(), process.fd()) {
            for fd_result in fdt {
                if let Ok(fd) = fd_result
                && let FDTarget::Socket(inode) = fd.target {
                    map.insert(inode, stat.clone());
                }
            }
        }
    }

    let tcp = procfs::net::tcp().unwrap();

    /*
    println!(
        "{:<26} {:<26} {:<15} {:<8} {}",
        "Local address", "Remote address", "State", "Inode", "PID/Program name"
    );
    */
    let mut connections = Vec::new();

    for entry in tcp.iter() {
        let local_address_string = entry.local_address.to_string();
        let local_address_split = local_address_string.split(':').collect::<Vec<_>>();
        let local_address = local_address_split[0].to_owned();
        let local_address_port = local_address_split[1].to_owned();

        let remote_address_string = entry.remote_address.to_string();
        let remote_address_split = remote_address_string.split(':').collect::<Vec<_>>();
        let remote_address = remote_address_split[0].to_owned();
        let remote_address_port = remote_address_split[1].to_owned();

        let state = format!("{:?}", entry.state);
        if let Some(stat) = map.get(&entry.inode) {
            /*
            println!(
                "{:<26} {:<26} {:<15} {:<12} {}/{}",
                local_address, remote_address, state, entry.inode, stat.pid, stat.comm
            );
            */
            let connection = Connection {
                local_address,
                local_address_port,
                remote_address,
                remote_address_port,
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