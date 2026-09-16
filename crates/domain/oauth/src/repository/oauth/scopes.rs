//! OAuth scope helpers.
//!
//! The scope table carries two policy bits: whether a scope is granted by
//! default, and whether a self-registering (RFC 7591) client may ask for it.
//! `admin` is never self-registrable; an operator grants it through the
//! admin client API.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::OAuthRepository;
use crate::error::{OauthError, OauthResult};

struct ScopeDefinition {
    name: &'static str,
    description: &'static str,
    is_default: bool,
    self_registrable: bool,
}

const VALID_SCOPES: &[ScopeDefinition] = &[
    ScopeDefinition {
        name: "user",
        description: "Standard user access",
        is_default: true,
        self_registrable: true,
    },
    ScopeDefinition {
        name: "admin",
        description: "Administrative access",
        is_default: false,
        self_registrable: false,
    },
    ScopeDefinition {
        name: "anonymous",
        description: "Anonymous user access",
        is_default: false,
        self_registrable: true,
    },
];

impl OAuthRepository {
    pub fn validate_scopes(requested_scopes: &[String]) -> OauthResult<Vec<String>> {
        if requested_scopes.is_empty() {
            return Ok(vec![]);
        }

        let mut valid_scopes = Vec::new();
        let mut invalid_scopes = Vec::new();

        for scope in requested_scopes {
            if Self::scope_exists(scope) {
                valid_scopes.push(scope.clone());
            } else {
                invalid_scopes.push(scope.clone());
            }
        }

        if !invalid_scopes.is_empty() {
            return Err(OauthError::Validation(format!(
                "Invalid scopes (roles): {}",
                invalid_scopes.join(", ")
            )));
        }

        Ok(valid_scopes)
    }

    pub fn validate_scopes_for_registration(
        requested_scopes: &[String],
    ) -> OauthResult<Vec<String>> {
        let valid = Self::validate_scopes(requested_scopes)?;
        let refused: Vec<&str> = valid
            .iter()
            .map(String::as_str)
            .filter(|scope| {
                !VALID_SCOPES
                    .iter()
                    .any(|def| def.name == *scope && def.self_registrable)
            })
            .collect();

        if !refused.is_empty() {
            return Err(OauthError::Validation(format!(
                "Scopes not available to self-registered clients: {}",
                refused.join(", ")
            )));
        }

        Ok(valid)
    }

    pub fn validate_scopes_for_client(
        client_scopes: &[String],
        requested_scopes: &[String],
    ) -> OauthResult<()> {
        let outside: Vec<&str> = requested_scopes
            .iter()
            .map(String::as_str)
            .filter(|scope| !client_scopes.iter().any(|c| c == scope))
            .collect();

        if !outside.is_empty() {
            return Err(OauthError::Validation(format!(
                "Scopes not registered for this client: {}",
                outside.join(", ")
            )));
        }

        Ok(())
    }

    pub fn get_available_scopes() -> Vec<(String, Option<String>)> {
        VALID_SCOPES
            .iter()
            .map(|def| (def.name.to_owned(), Some(def.description.to_owned())))
            .collect()
    }

    pub fn scope_exists(scope_name: &str) -> bool {
        VALID_SCOPES.iter().any(|def| def.name == scope_name)
    }

    pub fn parse_scopes(scope_string: &str) -> Vec<String> {
        scope_string
            .split_whitespace()
            .map(str::to_owned)
            .filter(|s| !s.is_empty())
            .collect()
    }

    pub fn format_scopes(scopes: &[String]) -> String {
        scopes.join(" ")
    }

    pub fn get_default_roles() -> Vec<String> {
        VALID_SCOPES
            .iter()
            .filter(|def| def.is_default)
            .map(|def| def.name.to_owned())
            .collect()
    }
}
