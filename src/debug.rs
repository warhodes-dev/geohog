use std::sync::{Arc, Mutex};

use clap::Parser;

use geohog::{
    config::Config,
    log, 
    net::{
        geolocate::GeolocationClient, get_tcp, Connection
    },
};

#[tokio::main]
async fn main() {

    let config = Config::parse();
    log::setup_trace(&config);

    let connections = get_tcp().unwrap();
    println!("=== Active TCP Connections ===");
    println!("{:<26} {:<26} {:<15} {:<8} {:<8} {}", "Local address", "Remote address", "State", "Inode", "PID", "Program name");
    for con in &connections {
        println!("{:<26} {:<26} {:<15} {:<8} {:<8} {}", 
            format!("{}:{}", con.local_address, con.local_address_port),
            format!("{}:{}", con.remote_address, con.remote_address_port),
            con.state,
            con.inode,
            con.pid,
            con.comm,
        );
    }

    async fn get_connections(loc_client: Arc<GeolocationClient>) -> Vec<Connection> {
        let mut connections = get_tcp().unwrap();
        for con in connections.iter_mut() {
            let remote_address = &con.remote_address;
            let mut geo_lock = con.geolocation.lock().unwrap();
            *geo_lock = loc_client.geolocate_ip_from_cache(remote_address).await;
        }
        connections
    }

    let mut handles = vec![];
    fn geolocate_connections<'a>(
        client: Arc<GeolocationClient>, 
        connections: impl Iterator<Item = &'a Connection>,
        handles: &mut Vec<tokio::task::JoinHandle<()>>
    ) {
        tracing::info!("Geolocating connections");
        for con in connections {
            let remote_address = con.remote_address.clone();
            let current_geo = &con.geolocation.lock().unwrap();
            if current_geo.is_none() {
                let handle = tokio::spawn({
                    let geo = Arc::clone(&con.geolocation);
                    let client = Arc::clone(&client);
                    async move {
                        if let Ok(new_geo) = client.geolocate_ip(&remote_address).await {
                            let mut geo_lock = geo.lock().unwrap();
                            *geo_lock = Some(new_geo);
                        }
                    }
                });
                handles.push(handle);
            }
        }
    }

    fn print_geolocations<'a>(connections: impl Iterator<Item = &'a Connection>) {
        println!("=== Geolocated Sockets ===");
        println!("{:<7} {:<20} {:<14} {:<12} {:<14} {:<12} {:<7} {:<25}", 
            "Socket", 
            "Remote address", 
            "City", 
            "Region", 
            "Country", 
            "Status",
            "PID",
            "Program Name"
        );
        for con in connections {
            let geolocation = con.geolocation.lock().unwrap();
            println!("{:<7} {:<20} {:<14} {:<12} {:<14} {:<12} {:<7} {:<25}", 
                con.local_address_port,
                format!("{}:{}", con.remote_address, con.remote_address_port),
                geolocation.as_ref().map_or("", |g| &g.city),
                geolocation.as_ref().map_or("", |g| &g.region),
                geolocation.as_ref().map_or("", |g| &g.country),
                con.state,
                con.pid,
                con.comm,
            );
        }
        println!();
    }

    let loc_client = Arc::new(GeolocationClient::new());

    let connections = get_connections(loc_client.clone()).await;

    print_geolocations(connections.iter());

    std::thread::sleep(std::time::Duration::from_millis(500));

    geolocate_connections(loc_client.clone(), connections.iter(), &mut handles);

    std::thread::sleep(std::time::Duration::from_millis(500));

    print_geolocations(connections.iter());


    for handle in handles {
        if let Err(e) = handle.await {
            tracing::error!("Task failed: {:?}", e);
        }
    }

}