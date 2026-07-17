//! Axum extractors for OAuth handler state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::FromRequestParts;
use axum::response::{IntoResponse, Response};
use http::request::Parts;
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::repository::OAuthRepository;

use super::OAuthHttpError;

#[derive(Debug)]
pub struct OAuthRepo(pub OAuthRepository);

impl FromRequestParts<OAuthState> for OAuthRepo {
    type Rejection = Response;

    #[expect(
        clippy::unused_async_trait_impl,
        reason = "async signature required by the FromRequestParts trait; this \
                  extractor constructs the repository synchronously"
    )]
    async fn from_request_parts(
        _parts: &mut Parts,
        state: &OAuthState,
    ) -> Result<Self, Self::Rejection> {
        OAuthRepository::new(state.db_pool())
            .map(OAuthRepo)
            .map_err(|e| {
                OAuthHttpError::server_error(format!("Repository initialization failed: {e}"))
                    .into_response()
            })
    }
}
