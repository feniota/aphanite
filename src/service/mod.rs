use crate::AppState;
use axum::Router;

mod yggdrasil;

pub fn router(state: AppState) -> axum::Router {
    Router::new()
        .with_state(state.clone())
        .nest("/api/yggdrasil", yggdrasil::router(state.clone()))
}
