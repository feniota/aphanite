use clap::Parser;
#[derive(Parser)]
pub struct Args {
    #[arg(short, long, help = "Enable verbose output")]
    pub verbose: bool,

    #[arg(long, help = "Enable debug output")]
    pub debug: bool,
}
