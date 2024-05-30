use std::sync::Arc;

use clap::Parser;

use geohog::{
    config::Config,
    log, 
    net::{
        get_tcp,
        geolocate::geolocate_ip,
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

    let mut handles = vec![];
    for con in &connections {
        let remote_address = con.remote_address.clone();
        let current_geo = &con.geolocation.lock().unwrap();
        if current_geo.is_none() {
            let handle = tokio::spawn({
                let geo = Arc::clone(&con.geolocation);
                async move {
                    if let Ok(new_geo) = geolocate_ip(remote_address).await {
                        let mut geo_lock = geo.lock().unwrap();
                        *geo_lock = Some(new_geo);
                    }
                }
            });
            handles.push(handle);
        }
    }

    fn print_geolocations(connections: &Vec<geohog::net::Connection>) {
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
    }

    print_geolocations(&connections);
    std::thread::sleep(std::time::Duration::from_millis(1000));
    print_geolocations(&connections);

    for handle in handles {
        if let Err(e) = handle.await {
            tracing::error!("Task failed: {:?}", e);
        }
    }

}