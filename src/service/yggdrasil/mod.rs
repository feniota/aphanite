//! Yggdrasil (Minecraft authentication server) API implementation

use crate::AppState;
use crate::service::yggdrasil::api::{
    authenticate, delete_texture, has_joined, invalidate, join, meta, minecraft, profile,
    put_texture, refresh, signout, validate,
};
use axum::Router;
use axum::routing::{delete, get, post, put};

pub mod api;
pub mod types;

pub fn router() -> Router<AppState> {
    use axum::handler::Handler;
    Router::new()
        .route("/", get(meta))
        .route("/authserver/authenticate", post(authenticate))
        .route("/authserver/refresh", post(refresh))
        .route("/authserver/validate", post(validate))
        .route("/authserver/invalidate", post(invalidate))
        .route("/authserver/signout", post(signout))
        .route("/sessionserver/session/minecraft/join", post(join))
        .route(
            "/sessionserver/session/minecraft/hasJoined",
            get(has_joined),
        )
        .route(
            "/sessionserver/session/minecraft/profile/{uuid}",
            get(profile),
        )
        .route("/api/profiles/minecraft", post(minecraft))
        .route(
            "/api/user/profile/{uuid}/{texture_type}",
            put(put_texture.layer(axum::extract::DefaultBodyLimit::max(9 * 1024 * 1024))),
        )
        .route(
            "/api/user/profile/{uuid}/{texture_type}",
            delete(delete_texture),
        )
        .fallback(fallback)
}

async fn fallback(uri: axum::http::Uri) -> axum::response::Response {
    use axum::response::IntoResponse;
    tracing::info!(
        "Incoming request to Yggdrasil api with an unknown url: {}",
        uri
    );
    api::YggdrasilError::http(404).into_response()
}
