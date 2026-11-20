//! Yggdrasil (Minecraft authentication server) API implementation

use crate::AppState;
use crate::service::yggdrasil::api::{
    authenticate, delete_texture, get_meta, get_profile, has_joined, invalidate, join,
    meta_without_slash, minecraft, put_texture, refresh, signout, validate,
};
use ntex::web;

pub mod api;
pub mod types;

pub fn configure(cfg: &mut web::ServiceConfig<web::AppState<AppState>>) {
    cfg.service(get_meta)
        .service(meta_without_slash)
        .service(authenticate)
        .service(refresh)
        .service(validate)
        .service(invalidate)
        .service(signout)
        .service(join)
        .service(has_joined)
        .service(get_profile)
        .service(minecraft)
        .service(put_texture)
        .service(delete_texture);
}

pub(super) async fn fallback(request: web::HttpRequest) -> web::HttpResponse {
    use crate::service::IntoResponse;
    tracing::info!(
        "Incoming request to Yggdrasil api with an unknown url: {}",
        request.uri()
    );
    api::YggdrasilError::http(404).into_response()
}
