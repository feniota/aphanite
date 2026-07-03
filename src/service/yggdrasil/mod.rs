use axum::Router;

pub mod api;
mod types;

pub fn router(state: crate::State) -> axum::Router {
    Router::new().with_state(state)
}
