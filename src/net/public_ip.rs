use std::net::{IpAddr, Ipv4Addr};

use anyhow::Result;

pub async fn get_public_ip() -> Result<Ipv4Addr> {
    let ip = match public_ip::addr().await.unwrap() {
        IpAddr::V4(ipaddr) => ipaddr,
        IpAddr::V6(_) => { anyhow::bail!("IPV6 not supported") }
    };

    Ok(ip)
}