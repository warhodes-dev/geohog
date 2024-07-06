use std::sync::{Arc, Mutex};

use anyhow::Result;

use ipgeolocate::Locator;
use sysinfo;
use netstat2::{iterate_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, SocketInfo, TcpSocketInfo};

use geolocate::{GeolocationClient, GeolocationTask};

pub mod geolocate;
pub mod public_ip;

pub struct Connection {
    pub local_address: String,
    pub local_address_port: String,
    pub remote_address: String,
    pub remote_address_port: String,
    pub state: String,
    pub inode: u32,
    pub pid: Option<u32>,
    pub process_name: Option<String>,
    pub geolocation: Arc<Mutex<Option<Locator>>>
}

impl Connection {
    fn new(tcp: &TcpSocketInfo, socket: &SocketInfo, pid: Option<u32>) -> Self {
        Connection {
            local_address: tcp.local_addr.to_string(),
            local_address_port: tcp.local_port.to_string(),
            remote_address: tcp.remote_addr.to_string(),
            remote_address_port: tcp.remote_port.to_string(),
            state: tcp.state.to_string(),
            inode: socket.inode,
            pid,
            process_name: None,
            geolocation: Arc::new(Mutex::new(None)),
        }
    }

    fn with_pid(tcp: &TcpSocketInfo, socket: &SocketInfo, pid: u32) -> Self {
        Connection::new(&tcp, &socket, Some(pid))
    }

    fn without_pid(tcp: &TcpSocketInfo, socket: &SocketInfo) -> Self {
        Connection::new(&tcp, &socket, None)
    }

    pub fn display(&self) -> ConnectionDisplay {
        todo!()
    }
}

pub struct ConnectionDisplay { /* TODO: fill this out */ }

pub struct NetClient {
    connections: Vec<Connection>,
    geolocation_client: GeolocationClient,
    sysinfo: sysinfo::System,
    runtime: tokio::runtime::Handle,
}

impl NetClient {
    pub fn new(runtime: &tokio::runtime::Runtime) -> Self {
        let connections = vec![];
        let geolocation_client = GeolocationClient::new(runtime.handle().clone());
        let sysinfo = sysinfo::System::new();
        let handle = runtime.handle().clone();
        NetClient {
            connections,
            geolocation_client,
            sysinfo,
            runtime: handle,
        }
    }

    pub fn connections<'a>(&'a self) -> impl Iterator<Item = &'a Connection> {
        self.connections.iter()
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.get_net_connections()?;
        self.restore_geolocations();
        self.refresh_proc_names();

        Ok(())
    }

    pub fn geolocate_connections(&mut self) -> Result<()> {
        let geolocation_tasks = self.connections.iter()
            .filter_map(|con| {
                // Filter out all already-geolocated connections
                let current_geolocation = &con.geolocation.lock().unwrap();
                if current_geolocation.is_none() {
                    let ip = con.remote_address.to_owned();
                    let locator = Arc::clone(&con.geolocation);
                    Some( GeolocationTask::new(ip, locator) )
                } else {
                    None
                }
            });
        self.geolocation_client.geolocate_ips(geolocation_tasks)?;
        Ok(())
    }

    fn restore_geolocations(&mut self) {
        tracing::debug!("Restoring geolocations from cache");

        for con in self.connections.iter_mut() {
            let remote_address = &con.remote_address;
            let mut geo_lock = con.geolocation.lock().unwrap();
            *geo_lock = self.geolocation_client.cache_get(remote_address);
        }
    }

    fn get_net_connections(&mut self) -> Result<()> {
        tracing::debug!("Getting network connections");

        let af_flags = AddressFamilyFlags::IPV4; // IPV6 is not supported
        let proto_flags = ProtocolFlags::TCP; // UDP is not supported
        let tcp_sockets_info = iterate_sockets_info(af_flags, proto_flags)?
            .flatten()
            .filter_map(|socket| match socket.protocol_socket_info {
                ProtocolSocketInfo::Tcp(ref tcp) => Some((tcp.clone(), socket)),
                _ => None
            });

        // Build connection list from scratch
        self.connections.clear();
        for (tcp, socket) in tcp_sockets_info {
            if socket.associated_pids.is_empty() {
                let connection = Connection::without_pid(&tcp, &socket);
                self.connections.push(connection);
            } else { 
                for pid in socket.associated_pids.iter() {
                    let connection = Connection::with_pid(&tcp, &socket, *pid);
                    self.connections.push(connection);
                }
            }
        }

        Ok(())
    }

    fn refresh_proc_names(&mut self) {
        tracing::debug!("Refreshing process names from PIDs");

        let all_pids = self.connections
            .iter()
            .filter_map(|conn| conn.pid)
            .map(|pid| sysinfo::Pid::from_u32(pid))
            .collect::<Vec<sysinfo::Pid>>();

        self.sysinfo.refresh_pids_specifics(
            all_pids.as_slice(),
            sysinfo::ProcessRefreshKind::new() // with_nothing()
        );

        for connection in self.connections.iter_mut() {
            if let Some(pid) = connection.pid {
                let pid = sysinfo::Pid::from_u32(pid);
                let proc = self.sysinfo.process(pid);
                connection.process_name = proc.map(|p| p.name().to_owned());
            }
        }
    }
}