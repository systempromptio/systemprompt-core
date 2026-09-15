//! Registered-redirect resolution for authorization-endpoint errors.
//!
//! RFC 6749 §4.1.2.1: the server must not redirect the user-agent to a
//! `redirect_uri` it has not confirmed is registered for `client_id`. A
//! [`RegisteredRedirect`] therefore exists only once that check has passed;
//! every earlier or failing validation renders the error page instead.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::AuthorizeQuery;
use crate::routes::oauth::OAuthHttpError;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::services::validation::validate_redirect_uri;

/// A `redirect_uri` confirmed to be registered for the request's client.
#[derive(Debug, Clone)]
pub struct RegisteredRedirect {
    uri: String,
    state: Option<String>,
}

impl RegisteredRedirect {
    #[must_use]
    pub fn attach(&self, err: OAuthHttpError) -> OAuthHttpError {
        err.with_redirect(self.uri.clone(), self.state.clone())
    }

    #[must_use]
    pub fn attach_if_registered(this: Option<&Self>, err: OAuthHttpError) -> OAuthHttpError {
        match this {
            Some(redirect) => redirect.attach(err),
            None => err,
        }
    }
}

pub async fn resolve_registered_redirect(
    repo: &OAuthRepository,
    params: &AuthorizeQuery,
) -> Result<Option<RegisteredRedirect>, OAuthHttpError> {
    let Some(requested) = params.redirect_uri.as_deref() else {
        return Ok(None);
    };
    if params.client_id.as_str().is_empty() {
        return Ok(None);
    }
    let client = repo
        .find_client_by_id(&params.client_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to look up OAuth client for redirect check");
            OAuthHttpError::server_error("Failed to resolve client")
        })?;
    let Some(client) = client else {
        return Ok(None);
    };
    let registered = validate_redirect_uri(&client.redirect_uris, Some(requested))
        .ok()
        .map(|uri| RegisteredRedirect {
            uri,
            state: params.state.clone(),
        });
    Ok(registered)
}
