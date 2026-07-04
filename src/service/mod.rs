//! The core Web service

use crate::AppState;
use axum::Router;

pub mod phenocryst;
pub mod yggdrasil;

pub fn router(state: AppState) -> Router {
    Router::new()
        .with_state(state.clone())
        .nest("/api/yggdrasil", yggdrasil::router(state.clone()))
}
