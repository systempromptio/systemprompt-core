//! Validation of the limits and scope bindings requested for an API key.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::attribution::ScopeBinding;

use crate::error::{Result, UserError};
use crate::models::ApiKeyLimits;

pub(super) fn validate_limits(limits: &ApiKeyLimits) -> Result<()> {
    if limits
        .model_allowlist
        .as_ref()
        .is_some_and(|models| models.is_empty() || models.iter().any(|m| m.trim().is_empty()))
    {
        return Err(UserError::Validation(
            "model_allowlist must name at least one model, or be omitted".into(),
        ));
    }
    if limits.budget_microdollars.is_some_and(|b| b < 0)
        || limits.max_requests.is_some_and(|m| m < 0)
    {
        return Err(UserError::Validation(
            "budget_microdollars and max_requests must not be negative".into(),
        ));
    }
    let ceiling = limits.budget_microdollars.is_some() || limits.max_requests.is_some();
    if ceiling && limits.request_window_seconds.is_none_or(|w| w <= 0) {
        return Err(UserError::Validation(
            "budget_microdollars and max_requests need a positive request_window_seconds".into(),
        ));
    }
    Ok(())
}

pub(super) fn validate_scopes(scopes: &[ScopeBinding]) -> Result<()> {
    for (index, scope) in scopes.iter().enumerate() {
        if scope.value.trim().is_empty() {
            return Err(UserError::Validation(format!(
                "scope {} must bind a non-empty value",
                scope.dimension
            )));
        }
        if scopes[..index]
            .iter()
            .any(|s| s.dimension == scope.dimension)
        {
            return Err(UserError::Validation(format!(
                "scope {} is bound more than once",
                scope.dimension
            )));
        }
    }
    Ok(())
}
