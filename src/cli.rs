use clap::Parser;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Parser)]
pub struct Args {
    #[arg(short, long, help = "Enable verbose output")]
    pub verbose: bool,

    #[arg(long, help = "Enable debug output")]
    pub debug: bool,
}

// Execute argument-specific logics. If the arguments would prevent Aphanite from starting, cli() should std::process::exit on its own.
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
    if args.debug {
        subscriber.with_file(true).pretty().finish().init();
    } else {
        subscriber.finish().init();
    }
}
