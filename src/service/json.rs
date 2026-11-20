//! JSON body extractor aligned with the rejection status codes axum produced.

use ntex::http::{Payload, StatusCode};
use ntex::web::error::JsonPayloadError;
use ntex::web::types::Json as NtexJson;
use ntex::web::{FromRequest, HttpRequest, State};
use serde::de::DeserializeOwned;

/// Drop-in replacement for [`ntex::web::types::Json`] as an extractor.
pub struct Json<T>(pub T);

impl<St, T> FromRequest<St> for Json<T>
where
    St: State,
    T: DeserializeOwned + 'static,
{
    type Error = crate::service::Error;

    async fn from_request(
        st: &St,
        req: &HttpRequest,
        payload: &mut Payload,
    ) -> Result<Self, Self::Error> {
        NtexJson::<T>::from_request(st, req, payload)
            .await
            .map(|NtexJson(data)| Json(data))
            .map_err(|error| {
                let status = match &error {
                    // Well-formed JSON that fails to deserialize (e.g. a
                    // malformed UUID) is a 422, not a 400.
                    JsonPayloadError::Deserialize(err)
                        if err.classify() == serde_json::error::Category::Data =>
                    {
                        StatusCode::UNPROCESSABLE_ENTITY
                    }
                    JsonPayloadError::ContentType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    JsonPayloadError::Overflow => StatusCode::PAYLOAD_TOO_LARGE,
                    _ => StatusCode::BAD_REQUEST,
                };
                crate::service::Error::new(status, error.to_string())
            })
    }
}
