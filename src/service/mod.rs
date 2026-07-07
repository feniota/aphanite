//! The core Web service

use crate::AppState;
use axum::Router;

pub mod phenocryst;
pub mod yggdrasil;

pub fn router(state: AppState) -> Router {
    use axum::routing::get;
    Router::new()
        .route("/api/yggdrasil/", get(yggdrasil::api::meta))
        .with_state(state.clone())
        .nest("/api/yggdrasil", yggdrasil::router(state.clone()))
}
