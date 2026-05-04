//! Lightweight value-validation traits used by config and metadata types.

use std::fmt::Debug;

/// Single field-scoped validation failure.
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// Name of the field that failed validation.
    pub field: String,
    /// Human-readable description of the failure.
    pub message: String,
    /// Optional contextual hint (path, surrounding object, ...).
    pub context: Option<String>,
}

impl ValidationError {
    /// Construct a new validation error for `field`.
    #[must_use]
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
            context: None,
        }
    }

    /// Attach contextual information to the error.
    #[must_use]
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref ctx) = self.context {
            write!(
                f,
                "VALIDATION ERROR [{}]: {} (context: {})",
                self.field, self.message, ctx
            )
        } else {
            write!(f, "VALIDATION ERROR [{}]: {}", self.field, self.message)
        }
    }
}

impl std::error::Error for ValidationError {}

/// Result alias for [`Validate`] / [`MetadataValidation`].
pub type ValidationResult<T> = Result<T, ValidationError>;

/// Self-validating value.
pub trait Validate: Debug {
    /// Run all validation rules and report the first failure.
    fn validate(&self) -> ValidationResult<()>;
}

/// Validation helper for types that can describe their required string
/// fields declaratively.
pub trait MetadataValidation: Validate {
    /// Return `(field_name, current_value)` pairs for every field that
    /// must be non-empty.
    fn required_string_fields(&self) -> Vec<(&'static str, &str)>;

    /// Default implementation that walks [`Self::required_string_fields`]
    /// and reports the first empty field.
    fn validate_required_fields(&self) -> ValidationResult<()> {
        for (field_name, field_value) in self.required_string_fields() {
            if field_value.is_empty() {
                return Err(ValidationError::new(
                    field_name,
                    format!("{field_name} cannot be empty"),
                ));
            }
        }
        Ok(())
    }
}
