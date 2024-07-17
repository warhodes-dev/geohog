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
            println!(
                "{:<7} {:<20} {:<14} {:<12} {:<14} {:<12} {:<7} {:<15}",
                con.local_address_port,
                format!("{}:{}", con.remote_address, con.remote_address_port),
                geolocation.as_ref().map_or("", |g| &g.city),
                geolocation.as_ref().map_or("", |g| &g.region),
                geolocation.as_ref().map_or("", |g| &g.country),
                con.state,
                con.processes.first().map_or("".to_owned(), |p| p.pid.to_string()),
                con.processes.first().map(|p| &p.name).cloned().flatten().map_or("".to_owned(), |s| s),
            );
        }
        println!();
    }

    let mut netstat = NetClient::new();

    for _ in 0..2 {
        println!("------------ *** Refreshing Socket Table *** ------------");
        netstat.refresh().unwrap();

        print_geolocations(netstat.connections());

        std::thread::sleep(std::time::Duration::from_millis(500));

        //netstat.geolocate_connections();

        std::thread::sleep(std::time::Duration::from_millis(500));

        print_geolocations(netstat.connections());
    }
}
