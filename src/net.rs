use std::{borrow::BorrowMut, cell::RefCell, net::{IpAddr, Ipv4Addr}};

use anyhow::{bail, Result};

use netstat2::{
    iterate_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, SocketInfo,
    TcpSocketInfo,
};
use sysinfo::{self, ProcessRefreshKind};

use geolocate::{Geolocation, GeolocationClient, Locator};
use tokio::sync::{broadcast, mpsc};

pub mod public_ip;
pub mod geolocate;

pub struct Netstat {
    connections: Vec<Connection>,
    sysinfo: sysinfo::System,
    geolocation_client: GeolocationClient,
}

impl Netstat {
    pub fn new() -> Self {
        let connections = vec![];
        let sysinfo = sysinfo::System::new();
        let geolocation_client = GeolocationClient::new();
        Netstat {
            connections,
            sysinfo,
            geolocation_client,
        }
    }

    pub fn connections<'a>(&'a self) -> impl Iterator<Item = &'a Connection> {
        self.connections.iter()
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.get_net_connections()?;
        Ok(())
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
            if let Ok(connection) = Connection::new(&mut self.sysinfo, &tcp, &socket) {
                self.connections.push(connection)
            }
        }

        Ok(())
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
}

impl Connection {
    fn new(
        sysinfo: &mut sysinfo::System, 
        tcp: &TcpSocketInfo, 
        socket: &SocketInfo
    ) -> Result<Self> {
        let remote_ip = to_ipv4(tcp.remote_addr)?;

        // TODO: Support these, just don't query the API for them
        if !remote_ip.is_global() {
            bail!("Non-global IP. Ignoring.");
        }

        let processes = get_process_info(sysinfo, &socket.associated_pids);

        return Ok(Connection {
            local_address: to_ipv4(tcp.local_addr)?,
            local_address_port: tcp.local_port,
            remote_address: to_ipv4(tcp.remote_addr)?,
            remote_address_port: tcp.remote_port,
            state: tcp.state.to_string(),
            #[cfg(any(target_os = "linux", target_os = "android"))]
            inode: socket.inode,
            processes,
        });

        fn to_ipv4(ip: IpAddr) -> Result<Ipv4Addr> {
            match ip {
                IpAddr::V4(ip) => Ok(ip),
                IpAddr::V6(ip) => match ip.to_canonical() {
                    IpAddr::V4(ip) => Ok(ip),
                    IpAddr::V6(_) => bail!("Canonicalizing Ipv6 -> Ipv4"),
                },
            }
        }
    }

    pub fn geolocation(&self, geo_client: &mut GeolocationClient) -> Option<Locator> {
        geo_client.geolocate(&self.remote_address)
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

fn get_process_info(sysinfo: &mut sysinfo::System, pids: &[u32]) -> Vec<Process> {
    let pids = pids
        .iter()
        .map(|pid_u32| sysinfo::Pid::from_u32(*pid_u32))
        .collect::<Vec<sysinfo::Pid>>();

    sysinfo.refresh_pids_specifics(&pids, ProcessRefreshKind::new()); // essentially `.with_nothing()`

    pids.iter()
        .map(|pid| {
            let proc = sysinfo.process(*pid);
            Process {
                pid: proc.map_or(pid.as_u32(), |proc| proc.pid().as_u32()),
                name: proc.map(|proc| proc.name().to_owned()),
            }
        })
        .collect::<Vec<Process>>()
}