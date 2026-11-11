//! Declarative macros that generate the typed identifier and token newtypes.
//!
//! The two public macros — [`define_id!`] and [`define_token!`] — are
//! `#[macro_export]`ed at the crate root. This module exists to keep their
//! source files individually below the 300-line cohesion limit and to expose
//! the supporting helper macros under stable paths.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod checked;
mod helpers;
mod id;
mod token;

pub use crate::{
    __define_id_checked, __define_id_common, __define_id_uuid, __define_id_validated_conversions,
    define_id, define_token,
};

#[doc(hidden)]
pub fn validate_uuid(
    id_type: &'static str,
    value: &str,
) -> Result<(), crate::error::IdValidationError> {
    uuid::Uuid::parse_str(value).map_err(|e| crate::error::IdValidationError::uuid(id_type, e))?;
    Ok(())
}

pub const MAX_EXTERNAL_ID_LEN: usize = 2048;

#[doc(hidden)]
pub fn validate_external_id(
    id_type: &'static str,
    value: &str,
) -> Result<(), crate::error::IdValidationError> {
    if value.is_empty() {
        return Err(crate::error::IdValidationError::empty(id_type));
    }
    if value.len() > MAX_EXTERNAL_ID_LEN {
        return Err(crate::error::IdValidationError::invalid(
            id_type,
            format!("exceeds {MAX_EXTERNAL_ID_LEN} bytes"),
        ));
    }
    if value.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(crate::error::IdValidationError::invalid(
            id_type,
            "contains whitespace or control characters",
        ));
    }
    Ok(())
}

#[doc(hidden)]
pub fn validate_non_empty(
    id_type: &'static str,
    value: &str,
) -> Result<(), crate::error::IdValidationError> {
    if value.trim().is_empty() {
        return Err(crate::error::IdValidationError::empty(id_type));
    }
    Ok(())
}
