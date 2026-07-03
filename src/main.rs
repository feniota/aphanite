use clap::Parser;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;
mod cli;
mod yggdrasil;
fn main() -> anyhow::Result<()> {
    let matches = cli::Args::parse();
    let filter = std::env::var("RUST_LOG").unwrap_or(format!(
        "{crate_name}={log_level}",
        crate_name = env!("CARGO_PKG_NAME"),
        log_level = if matches.verbose {
            "info"
        } else if matches.debug {
            "debug"
        } else {
            "warn"
        }
    ));
    let env_filter = EnvFilter::new(filter);
    let subscriber = tracing_subscriber::fmt().with_env_filter(env_filter);
    if matches.debug {
        subscriber.with_file(true).pretty().finish().init();
    } else {
        subscriber.finish().init();
    }
    Ok(())
}
