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
