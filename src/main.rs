use clap::Parser;
mod cli;
mod service;

#[derive(Clone)]
struct State {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = cli::Args::parse();
    cli::cli(&args);

    let state = State {};
    let app = service::router(state);

    tracing::info!("Service listening on http://{}:{}", args.listen, args.port);
    eprintln!("Service listening on http://{}:{}", args.listen, args.port);

    let listener = tokio::net::TcpListener::bind((args.listen, args.port)).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
