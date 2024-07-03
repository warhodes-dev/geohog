use std::sync::Arc;

use clap::Parser;

use geohog::{
    config::Config,
    log, 
    net::{
        NetClient,
        Connection,
        geolocate::GeolocationClient
    },
};
use tokio::runtime::Runtime;

fn main() {

    let config = Config::parse();
    log::setup_trace(&config);

    fn print_geolocations<'a>(connections: impl Iterator<Item = &'a Connection>) {
        println!("=== Geolocated Sockets ===");
        println!("{:<7} {:<20} {:<14} {:<12} {:<14} {:<12} {:<7} {:<15} {:<25}", 
            "Socket", 
            "Remote address", 
            "City", 
            "Region", 
            "Country", 
            "Status",
            "PID",
            "Program Name",
            "Path"
        );
        for con in connections {
            let geolocation = con.geolocation.lock().unwrap();
            println!("{:<7} {:<20} {:<14} {:<12} {:<14} {:<12} {:<7} {:<15} {:<25}", 
                con.local_address_port,
                format!("{}:{}", con.remote_address, con.remote_address_port),
                geolocation.as_ref().map_or("", |g| &g.city),
                geolocation.as_ref().map_or("", |g| &g.region),
                geolocation.as_ref().map_or("", |g| &g.country),
                con.state,
                con.pid.as_ref().map_or("".to_owned(), |pid| pid.to_string()),
                con.process.as_ref().map_or("".to_owned(), |proc| proc.name.as_ref().map_or("".to_owned(), |name| name.to_string())),
                con.process.as_ref().map_or("".to_owned(), |proc| proc.exe.as_ref().map_or("".to_owned(), |name| name.to_string())),
            );
        }
        println!();
    }

    let runtime = Runtime::new().unwrap();

    let mut netstat = NetClient::new(&runtime);

    for _ in 0..2 {
        netstat.refresh().unwrap();

        print_geolocations(netstat.connections());

        std::thread::sleep(std::time::Duration::from_millis(500));

        netstat.geolocate_connections();

        std::thread::sleep(std::time::Duration::from_millis(500));

        print_geolocations(netstat.connections());

        println!("\n\n*** Refreshing Socket Table ***\n\n")

    }

    runtime.shutdown_background();

}