//! Axum extractors for OAuth handler state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::FromRequestParts;
use axum::response::Response;
use http::request::Parts;
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::repository::OAuthRepository;


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
        Ok(OAuthRepo(state.oauth_repository().clone()))
    }
}
