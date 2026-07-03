use axum::Router;

pub mod api;
mod types;

pub fn router() -> axum::Router {
    Router::new()
}
