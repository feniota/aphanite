use aphanite::start;

#[tokio::main]
async fn main() {
    if let Err(e) = start().await {
        tracing::error!("Error occurred! Details: {}", e);
        std::process::exit(1);
    }
}
