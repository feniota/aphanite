use aphanite::start;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[ntex::main]
async fn main() {
    async_compat::set_fallback_runtime(|builder| {
        builder.worker_threads(2).name("toasty-worker").enable_all();
    })
    .expect("Failed to configure Tokio compatibility runtime");

    if let Err(e) = start().await {
        tracing::error!("Error occurred! Details: {}", e);
        std::process::exit(1);
    }
}
