use crate::AppState;
use crate::service::yggdrasil::api::{
    authenticate, delete_texture, has_joined, invalidate, join, meta, minecraft, profile,
    put_texture, refresh, signout, validate,
};
use axum::Router;
use axum::routing::{delete, get, post, put};

pub mod api;
mod types;

pub fn router(state: AppState) -> Router {
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
        .route("/api/user/profile/{uuid}/{textureType}", put(put_texture))
        .route(
            "/api/user/profile/{uuid}/{textureType}",
            delete(delete_texture),
        )
        .with_state(state)
}
