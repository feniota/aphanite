use crate::service::yggdrasil::api::{authenticate, refresh};
use crate::AppState;
use axum::routing::post;
use axum::Router;

pub mod api;
mod types;

pub fn router(state: crate::AppState) -> Router<AppState> {
    Router::new()
        .with_state(state)
        .route("/authserver/authenticate", post(authenticate))
        .route("/authserver/refresh", post(refresh))
        .into()
}
