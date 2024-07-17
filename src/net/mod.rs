use std::{
    net::{IpAddr, Ipv4Addr},
    sync::Arc,
    sync::Mutex,
};

use anyhow::{bail, Result};

use ipgeolocate::Locator;
use netstat2::{
    iterate_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, SocketInfo,
    TcpSocketInfo,
};
use sysinfo::{self, ProcessRefreshKind};

use geolocate::GeolocationClient;

pub mod geolocate;
pub mod public_ip;

pub struct NetClient {
    connections: Vec<Connection>,
    geo_client: GeolocationClient,
    sysinfo: sysinfo::System,
}

impl NetClient {
    pub fn new() -> Self {
        let connections = vec![];
        let geo_client = GeolocationClient::new();
        let sysinfo = sysinfo::System::new();
        NetClient {
            connections,
            geo_client,
            sysinfo,
        }
    }

    pub fn connections<'a>(&'a self) -> impl Iterator<Item = &'a Connection> {
        self.connections.iter()
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.get_net_connections()?;
        //self.get_geolocations(); No need to use this after refactoring
        Ok(())
    }

    fn get_geolocations(&mut self) {
        for conn in self.connections.iter_mut() {
            if conn.geolocation.lock().unwrap().is_none() {
                conn.geolocation = self.geo_client.geolocate_ip(&conn.remote_address);
            }
        }
    }

    fn get_net_connections(&mut self) -> Result<()> {
        let af_flags = AddressFamilyFlags::IPV4; // IPV6 is not supported
        let proto_flags = ProtocolFlags::TCP; // UDP is not supported
        let tcp_sockets_info = iterate_sockets_info(af_flags, proto_flags)?
            .flatten()
            .filter_map(|socket| match socket.protocol_socket_info {
                ProtocolSocketInfo::Tcp(ref tcp) => Some((tcp.clone(), socket)),
                _ => None,
            });

        // Build connection list from scratch
        self.connections.clear();
        for (tcp, socket) in tcp_sockets_info {
            if let Ok(connection) = Connection::new(self, &tcp, &socket) {
                self.connections.push(connection)
            }
        }

        Ok(())
    }

    fn get_process_info(&mut self, pids: &[u32]) -> Vec<Process> {
        let pids = pids
            .iter()
            .map(|pid_u32| sysinfo::Pid::from_u32(*pid_u32))
            .collect::<Vec<sysinfo::Pid>>();

        self.sysinfo
            .refresh_pids_specifics(&pids, ProcessRefreshKind::new()); // essentially `.with_nothing()`

        pids.iter()
            .map(|pid| {
                let proc = self.sysinfo.process(*pid);
                Process {
                    pid: proc.map_or(pid.as_u32(), |proc| proc.pid().as_u32()),
                    name: proc.map(|proc| proc.name().to_owned()),
                }
            })
            .collect::<Vec<Process>>()
    }
}

pub struct Connection {
    pub local_address: Ipv4Addr,
    pub local_address_port: u16,
    pub remote_address: Ipv4Addr,
    pub remote_address_port: u16,
    pub state: String,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub inode: u32,
    pub processes: Vec<Process>,
    pub geolocation: Arc<Mutex<Option<Locator>>>,
}

impl Connection {
    fn new(
        net_client: &mut NetClient, 
        tcp: &TcpSocketInfo, 
        socket: &SocketInfo
    ) -> Result<Self> {
        let remote_ip = to_ipv4(tcp.remote_addr)?;

        let processes = net_client.get_process_info(&socket.associated_pids);
        let geolocation = net_client.geo_client.geolocate_ip(&remote_ip);

        return Ok(Connection {
            local_address: to_ipv4(tcp.local_addr)?,
            local_address_port: tcp.local_port,
            remote_address: to_ipv4(tcp.remote_addr)?,
            remote_address_port: tcp.remote_port,
            state: tcp.state.to_string(),
            #[cfg(any(target_os = "linux", target_os = "android"))]
            inode: socket.inode,
            processes,
            geolocation,
        });

        fn to_ipv4(ip: IpAddr) -> Result<Ipv4Addr> {
            match ip {
                IpAddr::V4(ip) => Ok(ip),
                IpAddr::V6(ip) => match ip.to_canonical() {
                    IpAddr::V4(ip) => Ok(ip),
                    IpAddr::V6(_) => bail!("IPV6 not supported"),
                },
            }
        }
    }

    pub fn display(&self) -> ConnectionDisplay {
        todo!()
    }
}

pub struct ConnectionDisplay {/* TODO: fill this out */}

pub struct Process {
    pub pid: u32,
    pub name: Option<String>,
}
