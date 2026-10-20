use aphanite::start;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() {
    if let Err(e) = start().await {
        tracing::error!("Error occurred! Details: {}", e);
        std::process::exit(1);
    }
}
