//! PKCE code-verifier checking for the authorization-code exchange.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::{OauthError, OauthResult};
use crate::models::PkceMethod;
use base64::Engine;
use subtle::ConstantTimeEq;

pub(super) fn verify_pkce(
    challenge: &str,
    method: Option<&str>,
    code_verifier: Option<&str>,
) -> OauthResult<()> {
    let verifier = code_verifier.ok_or_else(|| {
        tracing::warn!("Missing code_verifier for PKCE challenge");
        OauthError::Validation("Invalid authorization code".to_owned())
    })?;

    let method = method.ok_or_else(|| {
        tracing::warn!("Missing code_challenge_method for PKCE challenge");
        OauthError::Validation("Invalid authorization code".to_owned())
    })?;

    let computed_challenge = match method.parse::<PkceMethod>() {
        Ok(PkceMethod::S256) => {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(verifier.as_bytes());
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize())
        },
        Err(e) => {
            tracing::warn!(method = %method, error = %e, "Unsupported code_challenge_method");
            return Err(OauthError::Validation(
                "Invalid authorization code".to_owned(),
            ));
        },
    };

    let challenge_matches: bool = computed_challenge
        .as_bytes()
        .ct_eq(challenge.as_bytes())
        .into();
    if challenge_matches {
        Ok(())
    } else {
        tracing::warn!("PKCE validation failed");
        Err(OauthError::Validation(
            "Invalid authorization code".to_owned(),
        ))
    }
}
