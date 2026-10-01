//! Typed error surface for the authz crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::domain_error;
use thiserror::Error;

domain_error! {
    pub enum AuthzError {
        common: [repository, validation],

        #[error("invalid rule_type: {0}")]
        InvalidRuleType(String),

        #[error("invalid access value: {0}")]
        InvalidAccess(String),

        #[error("authz hook transport: {0}")]
        Hook(#[from] reqwest::Error),

        #[error("authz bootstrap: {0}")]
        Bootstrap(#[from] AuthzBootstrapError),

        #[error("failed to {action} {path}: {source}")]
        File {
            action: &'static str,
            path: String,
            #[source]
            source: std::io::Error,
        },

        #[error("failed to parse {path} as AccessControlConfig: {source}")]
        ConfigParse {
            path: String,
            #[source]
            source: serde_yaml::Error,
        },

        #[error(
            "marketplace '{marketplace}': access.rules rule_type '{rule_type}' is not a valid \
             subject dimension: {source}"
        )]
        MarketplaceRuleType {
            marketplace: String,
            rule_type: String,
            #[source]
            source: Box<AuthzError>,
        },

        #[error("invalid entity id: {0}")]
        InvalidEntityId(#[source] systemprompt_identifiers::error::IdValidationError),
    }
}

impl From<sqlx::Error> for AuthzError {
    fn from(err: sqlx::Error) -> Self {
        Self::Repository(systemprompt_models::errors::RepositoryError::from(err))
    }
}

pub type AuthzResult<T> = Result<T, AuthzError>;

#[derive(Debug, Error)]
pub enum AuthzBootstrapError {
    #[error(
        "governance.authz.hook.mode = webhook but `url` is missing or blank — refusing to start"
    )]
    MissingWebhookUrl,

    #[error("governance.authz.hook.url is invalid or unsafe: {0} — refusing to start")]
    InvalidWebhookUrl(#[source] systemprompt_models::net::OutboundUrlError),

    #[error(
        "governance.authz.hook.mode = unrestricted requires `acknowledgement` field equal to the \
         literal: {expected:?}"
    )]
    MissingUnrestrictedAcknowledgement { expected: &'static str },

    #[error(
        "governance.authz.hook.mode = extension but no extension hook was supplied via \
         AppContextBuilder::with_authz_hook(...) — refusing to start"
    )]
    ExtensionModeButNoHook,

    #[error(
        "an extension authz hook was supplied via AppContextBuilder::with_authz_hook(...) but \
         governance.authz.hook.mode is `{mode}` (must be `extension`) — refusing to start"
    )]
    ExtensionHookButWrongMode { mode: &'static str },

    #[error(
        "an extension authz hook was supplied via AppContextBuilder::with_authz_hook(...) but the \
         profile has no `governance.authz` block — set `governance.authz.hook.mode = extension` \
         or drop the `with_authz_hook` call"
    )]
    NoGovernanceButExtensionHook,
}
