//! Opaque request identifier echoed by upstream model providers.
//!
//! Captured for observability when a provider returns a trace token in
//! its response (e.g. via `x-context-id` or analogous headers). The shape
//! is provider-defined, so this id is only validated for non-empty and a
//! sane upper length bound.

use crate::error::IdValidationError;

const MAX_LEN: usize = 256;

fn validate(value: &str) -> Result<(), IdValidationError> {
    if value.is_empty() {
        return Err(IdValidationError::empty("ProviderRequestId"));
    }
    if value.len() > MAX_LEN {
        return Err(IdValidationError::invalid(
            "ProviderRequestId",
            "exceeds maximum length",
        ));
    }
    Ok(())
}

crate::define_id!(ProviderRequestId, validated, schema, validate);
