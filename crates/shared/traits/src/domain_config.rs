//! Domain configuration trait for startup validation.

use std::fmt::Debug;

use crate::context::ConfigProvider;
use crate::validation_report::ValidationReport;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DomainConfigError {
    #[error("Failed to load config: {0}")]
    LoadError(String),

    #[error("Config file not found: {0}")]
    NotFound(String),

    #[error("Failed to parse config: {0}")]
    ParseError(String),

    #[error("Validation failed: {0}")]
    ValidationError(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
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
