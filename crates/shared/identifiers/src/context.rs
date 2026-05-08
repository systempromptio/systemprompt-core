//! Execution-context identifier — UUID v4 only.

use crate::error::IdValidationError;

crate::define_id!(ContextId, validated, schema, validate_uuid_v4);

fn validate_uuid_v4(s: &str) -> Result<(), IdValidationError> {
    uuid::Uuid::parse_str(s).map_err(|e| IdValidationError::invalid("ContextId", e.to_string()))?;
    Ok(())
}

impl ContextId {
    pub fn generate() -> Self {
        // Safe: UUID v4 from `uuid` crate is always a valid UUID string.
        Self::new(uuid::Uuid::new_v4().to_string())
    }
}
