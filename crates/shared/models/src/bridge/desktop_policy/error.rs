//! Validation errors for the signed Claude Desktop fleet policy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[derive(Debug, thiserror::Error)]
pub enum DesktopPolicyError {
    #[error("desktop_policy requires schema_version: 1")]
    SchemaVersion,
    #[error("Desktop settings catalog: {0}")]
    Catalog(#[from] serde_json::Error),
    #[error("unsupported Desktop setting {0}; use the documented current spelling")]
    Unsupported(String),
    #[error("{0} is controlled by the Bridge or inapplicable to gateway mode")]
    Reserved(String),
    #[error("{setting} requires {expected}")]
    InvalidType { setting: String, expected: String },
    #[error("{0} is outside its documented range")]
    Range(String),
    #[error("{0} has an unsupported enum value")]
    EnumValue(String),
    #[error(
        "builtinToolPolicy decisions must be allow or ask; use disabledBuiltinTools to deny tools"
    )]
    ToolDecision,
    #[error("organizationInstructions must not exceed 3000 characters")]
    InstructionsLength,
    #[error("Desktop telemetry is metadata-only; otlpContentCapture must be empty")]
    ContentCapture,
    #[error("unsupported or invalid field {setting}.{field}")]
    Field { setting: String, field: String },
    #[error("inferenceModelPricing rows require a name and all four prices")]
    PricingFields,
    #[error("{0} rows require serverUrl")]
    ServerUrl(String),
}
