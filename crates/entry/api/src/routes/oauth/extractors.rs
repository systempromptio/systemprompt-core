use axum::extract::FromRequestParts;
use axum::response::Response;
use http::request::Parts;
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::repository::OAuthRepository;

use super::responses::init_error;

#[derive(Debug)]
pub struct OAuthRepo(pub OAuthRepository);

impl FromRequestParts<OAuthState> for OAuthRepo {
    type Rejection = Response;

    async fn from_request_parts(
        _parts: &mut Parts,
        state: &OAuthState,
    ) -> Result<Self, Self::Rejection> {
        OAuthRepository::new(state.db_pool())
            .map(OAuthRepo)
            .map_err(init_error)
    }
}
