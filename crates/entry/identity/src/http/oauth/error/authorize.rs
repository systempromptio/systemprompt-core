//! The OIDC Core §3.1.2.6 authentication-request errors and the RFC 9207
//! `iss` an authorization error response carries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{OAuthErrorCode, OAuthHttpError};

impl OAuthHttpError {
    #[must_use]
    pub fn login_required(description: impl Into<String>) -> Self {
        Self::new(OAuthErrorCode::LoginRequired, description)
    }

    #[must_use]
    pub fn consent_required(description: impl Into<String>) -> Self {
        Self::new(OAuthErrorCode::ConsentRequired, description)
    }

    #[must_use]
    pub fn interaction_required(description: impl Into<String>) -> Self {
        Self::new(OAuthErrorCode::InteractionRequired, description)
    }

    #[must_use]
    pub fn account_selection_required(description: impl Into<String>) -> Self {
        Self::new(OAuthErrorCode::AccountSelectionRequired, description)
    }

    #[must_use]
    pub fn with_redirect_issuer(mut self, issuer: impl Into<String>) -> Self {
        self.redirect_issuer = Some(issuer.into());
        self
    }
}
