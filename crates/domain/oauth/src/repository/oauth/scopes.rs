//! OAuth scope helpers.

use super::OAuthRepository;
use crate::error::{OauthError, OauthResult};

const VALID_SCOPES: &[(&str, &str, bool)] = &[
    ("user", "Standard user access", true),
    ("admin", "Administrative access", false),
    ("anonymous", "Anonymous user access", false),
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

    pub fn get_available_scopes() -> Vec<(String, Option<String>)> {
        VALID_SCOPES
            .iter()
            .map(|(name, desc, _)| ((*name).to_owned(), Some((*desc).to_owned())))
            .collect()
    }

    pub fn scope_exists(scope_name: &str) -> bool {
        VALID_SCOPES.iter().any(|(name, _, _)| *name == scope_name)
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
            .filter(|(_, _, is_default)| *is_default)
            .map(|(name, _, _)| (*name).to_owned())
            .collect()
    }
}
