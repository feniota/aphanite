use std::net::IpAddr;

use clap::Parser;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Parser)]
pub struct Args {
    /// Enable verbose output
    #[arg(short, long)]
    pub verbose: bool,

    /// Enable debug output
    #[arg(long)]
    pub debug: bool,

    /// The IP address to listen on
    #[arg(short, long, default_value_t=IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)))]
    pub listen: IpAddr,

    /// The port to listen on
    #[arg(short, long, default_value_t = 3000_u16)]
    pub port: u16,

    /// Directory to store the data
    #[arg(short, long, default_value_t = ToString::to_string("./data"))]
    pub data: String,

    /// Path to configuration file
    #[arg(short, long, default_value_t = ToString::to_string("./config.toml"))]
    pub config: String,
}

/// Execute argument-specific logics. If the arguments would prevent Aphanite from starting, cli() should std::process::exit on its own.
pub fn cli(args: &Args) {
    let filter = std::env::var("RUST_LOG").unwrap_or(format!(
        "{crate_name}={log_level}",
        crate_name = env!("CARGO_PKG_NAME"),
        log_level = if args.debug {
            "debug"
        } else if args.verbose {
            "info"
        } else {
            "warn"
        }
    ));
    let env_filter = EnvFilter::new(filter);
    let subscriber = tracing_subscriber::fmt().with_env_filter(env_filter);
    // For non-debug builds, hide code paths and make logs shorter even when --debug is specified
    if args.debug && cfg!(debug_assertions) {
        subscriber.with_file(true).pretty().finish().init();
    } else {
        subscriber.finish().init();
    }
}
