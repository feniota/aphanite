use aphanite::start;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[ntex::main]
async fn main() {
    // Load CryptoProvider
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install CryptoProvider");

    if let Err(e) = start().await {
        tracing::error!("Error occurred! Details: {}", e);
        std::process::exit(1);
    }
}
