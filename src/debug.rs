use std::ops::Deref;

use clap::Parser;

use geohog::{
    config::Config,
    log,
    net::{Connection, NetClient},
};

#[tokio::main]
async fn main() {
    let config = Config::parse();
    log::setup_trace(&config);

    fn print_geolocations<'a>(connections: impl Iterator<Item = &'a Connection>) {
        println!("=== Geolocated Sockets ===");
        println!(
            "{:<7} {:<20} {:<14} {:<12} {:<14} {:<12} {:<7} {:<15}",
            "Socket",
            "Remote address",
            "City",
            "Region",
            "Country",
            "Status",
            "PID",
            "Program Name",
        );
        for con in connections {
            let geolocation = &con.geolocation.lock().unwrap();
            let geo = match geolocation.deref() {
                geohog::net::geolocate::Locator::Some(geo) => Some(Some(geo)),
                geohog::net::geolocate::Locator::Refused(refusal) => Some(None),
                geohog::net::geolocate::Locator::Pending => None,
            };
            println!(
                "{:<7} {:<21} {:<14} {:<20} {:<14} {:<12} {:<7} {:<15}",
                con.local_address_port,
                format!("{}:{}", con.remote_address, con.remote_address_port),
                geo.as_ref().map_or("", |not_pending| not_pending.map_or("Refused", |g| &g.city)),
                geo.as_ref().map_or("", |not_pending| not_pending.map_or("Refused", |g| &g.region)),
                geo.as_ref().map_or("", |not_pending| not_pending.map_or("Refused", |g| &g.country)),
                con.state,
                con.processes.first().map_or("".to_owned(), |p| p.pid.to_string()),
                con.processes.first().map(|p| &p.name).cloned().flatten().map_or("".to_owned(), |s| s),
            );
        }
        println!();
    }

    let mut netstat = NetClient::new();

    loop {
        println!("\n\n------------ *** Refreshing Socket Table *** ------------\n\n");
        netstat.refresh().unwrap();

        for _ in 0..5 {
            print_geolocations(netstat.connections());
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        }
    }
}
