//! A member's typed profile configuration section.
//!
//! A member declares its section with `Contributions::config::<C>()`. The
//! declaration records the section's prefix, its JSON schema and a
//! monomorphised validator; the composition runs the validator against the
//! profile's section at admission, so a malformed section refuses the
//! composition rather than a request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::ExtensionConfigError;

/// A member's profile configuration section, deserialised and validated at
/// admission.
pub trait ExtensionConfig:
    serde::de::DeserializeOwned + schemars::JsonSchema + Send + Sync + 'static
{
    const PREFIX: &'static str;
    fn validate(&self) -> Result<(), ExtensionConfigError>;
}

type ValidateFn = fn(&serde_json::Value) -> Result<(), ExtensionConfigError>;

/// One declared configuration section.
#[derive(Clone, Copy)]
pub struct ConfigDecl {
    prefix: &'static str,
    schema: fn() -> schemars::Schema,
    validate: ValidateFn,
}

impl ConfigDecl {
    pub(crate) fn of<C: ExtensionConfig>() -> Self {
        Self {
            prefix: C::PREFIX,
            schema: schema_of::<C>,
            validate: validate_as::<C>,
        }
    }

    #[must_use]
    pub const fn prefix(&self) -> &'static str {
        self.prefix
    }

    #[must_use]
    pub fn schema(&self) -> schemars::Schema {
        (self.schema)()
    }

    // JSON: the member's section of the profile YAML; the member owns its shape.
    pub fn validate(&self, section: &serde_json::Value) -> Result<(), ExtensionConfigError> {
        (self.validate)(section)
    }
}

impl std::fmt::Debug for ConfigDecl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigDecl")
            .field("prefix", &self.prefix)
            .finish_non_exhaustive()
    }
}

fn schema_of<C: schemars::JsonSchema>() -> schemars::Schema {
    schemars::schema_for!(C)
}

fn validate_as<C: ExtensionConfig>(
    section: &serde_json::Value,
) -> Result<(), ExtensionConfigError> {
    let config = C::deserialize(section).map_err(|source| ExtensionConfigError::ParseError {
        source: Box::new(source),
    })?;
    config.validate()
}
