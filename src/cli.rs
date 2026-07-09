//! Command-line arguments definition and command exection

use std::net::IpAddr;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
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
    #[arg(short, long)]
    pub listen: Option<IpAddr>,

    /// The port to listen on
    #[arg(short, long)]
    pub port: Option<u16>,

    /// Path to configuration file
    #[arg(short, long, default_value_os_t = From::from("./config.toml"), global = true)]
    pub config: PathBuf,

    /// [Debug only] Create a test user when the server launches
    #[cfg(debug_assertions)]
    #[arg(long)]
    pub with_test_user: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Generate and write a configuration file
    Init,
    /// Create an admin user
    CreateAdmin {
        /// Email of the admin user
        #[arg(long)]
        email: String,

        /// Nickname of the admin user (defaults to email)
        #[arg(long)]
        nickname: Option<String>,

        /// Password of the admin user
        #[arg(long)]
        password: String,
    },
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

    if let Some(Command::Init) = &args.command {
        if let Err(e) = crate::config::AppConfig::init(args) {
            tracing::error!("Failed to initialize configuration: {e}");
            std::process::exit(1);
        }
        std::process::exit(0);
    }
}
