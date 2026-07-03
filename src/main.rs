use clap::Parser;
mod cli;
mod yggdrasil;
fn main() -> anyhow::Result<()> {
    let args = cli::Args::parse();
    cli::cli(&args);
    Ok(())
}
