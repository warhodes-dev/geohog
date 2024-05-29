use std::net::IpAddr;

use anyhow::Result;

pub async fn get_public_ip() -> Result<String> {
    let ip_raw = match public_ip::addr().await.unwrap() {
        IpAddr::V4(ipaddr) => ipaddr,
        IpAddr::V6(_) => { anyhow::bail!("IPV6 not supported") }
    };

    let ip = ip_raw.octets()
        .map(|byte| byte.to_string())
        .iter()
        .map(|s| s.as_ref())
        .intersperse(".")
        .collect::<String>();

    Ok(ip)
}