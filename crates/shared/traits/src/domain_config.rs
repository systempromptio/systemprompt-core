//! Per-domain configuration loading and validation.
//!
//! Each domain crate registers a [`DomainConfig`] implementation in the
//! [`DomainConfigRegistry`]; the runtime walks the registry at startup,
//! ordered by [`DomainConfig::priority`], to load and validate domain
//! configuration.

use std::fmt::Debug;

use crate::context::ConfigProvider;
use crate::validation_report::ValidationReport;

/// Errors returned by domain config implementations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DomainConfigError {
    /// The config file could not be loaded from disk or remote source.
    #[error("Failed to load config: {0}")]
    LoadError(String),

    /// The expected config file was not found.
    #[error("Config file not found: {0}")]
    NotFound(String),

    /// Parsing the loaded bytes failed.
    #[error("Failed to parse config: {0}")]
    ParseError(String),

    /// Validation failed after a successful parse.
    #[error("Validation failed: {0}")]
    ValidationError(String),

    /// Adapter for legacy `anyhow`-based call sites.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Per-domain config loader and validator.
pub trait DomainConfig: Send + Sync + Debug {
    /// Stable identifier for this domain (`agent`, `mcp`, ...).
    fn domain_id(&self) -> &'static str;

    /// Load the domain's configuration from the supplied
    /// [`ConfigProvider`].
    fn load(&mut self, config: &dyn ConfigProvider) -> Result<(), DomainConfigError>;

    /// Validate the previously-loaded configuration.
    fn validate(&self) -> Result<ValidationReport, DomainConfigError>;

    /// IDs of domains whose configuration must be loaded first.
    fn dependencies(&self) -> &[&'static str] {
        &[]
    }

    /// Lower numbers load earlier.
    fn priority(&self) -> u32 {
        100
    }
}

/// Aggregating registry of every [`DomainConfig`] implementation.
pub struct DomainConfigRegistry {
    validators: Vec<Box<dyn DomainConfig>>,
}

impl DomainConfigRegistry {
    /// Construct an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            validators: Vec::new(),
        }
    }

    /// Register a [`DomainConfig`] implementation.
    pub fn register(&mut self, validator: Box<dyn DomainConfig>) {
        self.validators.push(validator);
    }

    /// Return the registered validators sorted by [`DomainConfig::priority`].
    pub fn validators_sorted(&self) -> Vec<&dyn DomainConfig> {
        let mut validators: Vec<_> = self.validators.iter().map(AsRef::as_ref).collect();
        validators.sort_by_key(|v| v.priority());
        validators
    }

    /// Mutable iterator over registered validators in priority order.
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
