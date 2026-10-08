//! Scope dimension identifier.
//!
//! The name of a subject-attribute dimension (an extension `RuleType` slug
//! such as `project` or `cost_centre`) that a gateway request can be
//! attributed to. The value attributed for a dimension is the tenant's own
//! vocabulary and is carried as text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::IdValidationError;

crate::define_id!(ScopeDimension, checked, validate_scope_dimension);

const MAX_SCOPE_DIMENSION_LEN: usize = 64;

pub fn validate_scope_dimension(value: &str) -> Result<(), IdValidationError> {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return Err(IdValidationError::empty("ScopeDimension"));
    };
    if value.len() > MAX_SCOPE_DIMENSION_LEN {
        return Err(IdValidationError::invalid(
            "ScopeDimension",
            format!("must be at most {MAX_SCOPE_DIMENSION_LEN} characters"),
        ));
    }
    if !first.is_ascii_lowercase() {
        return Err(IdValidationError::invalid(
            "ScopeDimension",
            "must start with a lowercase letter",
        ));
    }
    if !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
        return Err(IdValidationError::invalid(
            "ScopeDimension",
            "may contain only lowercase letters, digits and '_'",
        ));
    }
    Ok(())
}
