use std::sync::{Arc, Mutex};
use netstat2::{iterate_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, SocketInfo, TcpSocketInfo};

use ipgeolocate::Locator;

use anyhow::Result;

use geolocate::GeolocationClient;
use sysinfo::{Pid, ProcessRefreshKind, System};

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
    fn new(
        tcp: &TcpSocketInfo, 
        socket: &SocketInfo, 
    ) -> Self {
        Connection {
            local_address: tcp.local_addr.to_string(),
            local_address_port: tcp.local_port.to_string(),
            remote_address: tcp.remote_addr.to_string(),
            remote_address_port: tcp.remote_port.to_string(),
            state: tcp.state.to_string(),
            inode: socket.inode,
            pid: None,
            process_name: None,
            geolocation: Arc::new(Mutex::new(None)),
        }
    }

    fn with_pid(mut self, net_client: &NetClient, pid: u32) -> Self {
        let process_name = get_proc_name(pid);
        self.pid = Some(pid);
        self.process_name = process_name;
        self
    }

    pub fn display(&self) -> ConnectionDisplay {
        todo!()
    }
}

pub struct ConnectionDisplay { /* TODO: fill this out */ }

pub struct NetClient {
    connections: Vec<Connection>,
    geolocation_client: Arc<GeolocationClient>,
    sysinfo: sysinfo::System,
    runtime: tokio::runtime::Handle,
}

impl NetClient {
    pub fn new(runtime: &tokio::runtime::Runtime) -> Self {
        let connections = vec![];
        let geolocation_client = Arc::new(GeolocationClient::new());
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
                let connection = Connection::new(&tcp, &socket);
                self.connections.push(connection);
            } else { for pid in socket.associated_pids.iter() {
                let connection = Connection::new(&tcp, &socket).with_pid(&self, *pid);
                self.connections.push(connection);
            }}
        }

        Ok(())
    }
}

//TODO: Change this to be a method of NetClient,
//      call 
fn get_proc_name(pid: u32) -> Option<String> {
    let mut sys = System::new();
    sys.refresh_process_specifics(
        Pid::from(pid as usize),
        ProcessRefreshKind::new()//with_nothing()
    );
    let proc = sys.process(sysinfo::Pid::from(pid as usize));

    let name = proc.map(|p| p.name().to_owned());

    name
}
