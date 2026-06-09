//! Per-domain configuration loading and validation.
//!
//! Each domain crate registers a [`DomainConfig`] implementation in the
//! [`DomainConfigRegistry`]; the runtime walks the registry at startup,
//! ordered by [`DomainConfig::priority`], to load and validate domain
//! configuration.

use std::fmt::Debug;

use crate::context::ConfigProvider;
use crate::validation_report::ValidationReport;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DomainConfigError {
    #[error("Failed to load config: {message}")]
    LoadError { message: String },

    #[error("Config file not found: {0}")]
    NotFound(String),

    #[error("Failed to parse config: {message}")]
    ParseError { message: String },

    #[error("Validation failed: {message}")]
    ValidationError { message: String },
}

pub trait DomainConfig: Send + Sync + Debug {
    fn domain_id(&self) -> &'static str;

    fn load(&mut self, config: &dyn ConfigProvider) -> Result<(), DomainConfigError>;

    fn validate(&self) -> Result<ValidationReport, DomainConfigError>;

    fn dependencies(&self) -> &[&'static str] {
        &[]
    }

    fn priority(&self) -> u32 {
        100
    }
}

pub struct DomainConfigRegistry {
    validators: Vec<Box<dyn DomainConfig>>,
}

impl DomainConfigRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            validators: Vec::new(),
        }
    }

    pub fn register(&mut self, validator: Box<dyn DomainConfig>) {
        self.validators.push(validator);
    }

    pub fn validators_sorted(&self) -> Vec<&dyn DomainConfig> {
        let mut validators: Vec<_> = self.validators.iter().map(AsRef::as_ref).collect();
        validators.sort_by_key(|v| v.priority());
        validators
    }

    pub fn validators_mut(&mut self) -> impl Iterator<Item = &mut Box<dyn DomainConfig>> {
        self.validators.sort_by_key(|v| v.priority());
        self.validators.iter_mut()
    }
}

impl Default for DomainConfigRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Debug for DomainConfigRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DomainConfigRegistry")
            .field("validator_count", &self.validators.len())
            .finish()
    }
}
