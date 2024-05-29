use clap::Parser;

use geohog::{
    config::Config,
    log, net::get_tcp,
};

#[tokio::main]
async fn main() {
    let config = Config::parse();
    log::setup_trace(&config);

    let connections = get_tcp().unwrap();
    println!("=== Active TCP Connections ===");
    println!("{:<26} {:<26} {:<15} {:<8} {:<8} {}", "Local address", "Remote address", "State", "Inode", "PID", "Program name");
    for con in connections {
        println!("{:<26} {:<26} {:<15} {:<8} {:<8} {}", 
            format!("{}:{}", con.local_address, con.local_address_port),
            format!("{}:{}", con.remote_address, con.remote_address_port),
            con.state,
            con.inode,
            con.pid,
            con.comm,
        );
    }
}