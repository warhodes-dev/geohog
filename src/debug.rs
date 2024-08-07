use clap::Parser;

use geohog::{
    config::Config,
    log,
    net::{ip_api::{self, Locator}, Netstat},
};

#[tokio::main]
async fn main() {
    let config = Config::parse();
    log::setup_trace(&config);

    fn print_geolocations<'a>(netstat: &mut Netstat, geo_client: &mut ip_api::Client) {
        println!("=== Geolocated Sockets ===");
        println!(
            "{:<7} {:<21} {:<14} {:<8} {:<8} {:<12} {:<7} {:<15}",
            "Socket",
            "Remote address",
            "City",
            "Region",
            "Country",
            "Status",
            "PID",
            "Program Name",
        );
        for con in netstat.connections() {
            let location = match geo_client.geolocate(&con.remote_address) {
                Some(Locator::Global(location)) => Some(location),
                _ => None,
            };
            println!(
                "{:<7} {:<21} {:<14} {:<8} {:<8} {:<12} {:<7} {:<15}",
                con.local_address_port,
                format!("{}:{}", con.remote_address, con.remote_address_port),
                location.as_ref().map_or("", |g| &g.city),
                location.as_ref().map_or("", |g| &g.region_code),
                location.as_ref().map_or("", |g| &g.country_code),
                con.state,
                con.processes.first().map_or("".to_owned(), |p| p.pid.to_string()),
                con.processes.first().map(|p| &p.name).cloned().flatten().map_or("".to_owned(), |s| s),
            );
        }
        println!();
    }

    let mut netstat = Netstat::new();
    let mut geo_client = ip_api::Client::new();

    loop {
        println!("\n\n------------ *** Refreshing Socket Table *** ------------\n\n");
        netstat.refresh().unwrap();
        netstat.connections()
            .for_each(|conn| {
                geo_client.request(conn.remote_address);
            });

        for _ in 0..5 {
            print_geolocations(&mut netstat, &mut geo_client);
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        }
    }
}
