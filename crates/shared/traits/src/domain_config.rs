//! Domain configuration trait for startup validation.
//!
//! All domain crates MUST implement this trait to participate in
//! startup validation. This ensures consistent config loading and
//! validation across the entire system.

use std::fmt::Debug;

use crate::context::ConfigProvider;
use crate::validation_report::ValidationReport;

/// Error type for domain config operations.
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

/// Trait for domain configuration validation.
///
/// # Implementation Requirements
///
/// Every domain crate MUST implement this trait. The implementation should:
///
/// 1. **`domain_id()`**: Return a unique identifier for the domain
/// 2. **`load()`**: Load and parse config files, storing state internally
/// 3. **`validate()`**: Run semantic validation on loaded config
/// 4. **`dependencies()`**: Declare any domains that must load first
///
/// # Example
///
/// ```rust,ignore
/// use systemprompt_traits::{ConfigProvider, DomainConfig, DomainConfigError, ValidationReport};
/// use systemprompt_traits::validation_report::ValidationError;
///
/// pub struct ContentConfigValidator {
///     config: Option<ContentConfig>,
/// }
///
/// impl DomainConfig for ContentConfigValidator {
///     fn domain_id(&self) -> &'static str {
///         "content"
///     }
///
///     fn load(&mut self, config: &dyn ConfigProvider) -> Result<(), DomainConfigError> {
///         let path = config.system_path(); // Use ConfigProvider methods
///         let content = std::fs::read_to_string(path)
///             .map_err(|e| DomainConfigError::LoadError(e.to_string()))?;
///
///         let parsed: ContentConfig = serde_yaml::from_str(&content)
///             .map_err(|e| DomainConfigError::ParseError(e.to_string()))?;
///
///         self.config = Some(parsed);
///         Ok(())
///     }
///
///     fn validate(&self) -> Result<ValidationReport, DomainConfigError> {
///         let config = self.config.as_ref()
///             .ok_or_else(|| DomainConfigError::ValidationError("Not loaded".into()))?;
///
///         let mut report = ValidationReport::new("content");
///
///         // Semantic validation
///         for source in &config.sources {
///             if !std::path::Path::new(&source.path).exists() {
///                 report.add_error(ValidationError::new(
///                     format!("sources.{}", source.name),
///                     "Source directory does not exist",
///                 ).with_path(&source.path));
///             }
///         }
///
///         Ok(report)
///     }
/// }
/// ```
pub trait DomainConfig: Send + Sync + Debug {
    /// Unique identifier for this domain.
    ///
    /// Used in validation reports and error messages.
    /// Examples: "web", "content", "agents", "mcp"
    fn domain_id(&self) -> &'static str;

    /// Load configuration from the given config.
    ///
    /// This method should:
    /// 1. Read the config file(s) from paths in `config`
    /// 2. Parse the content (YAML, JSON, etc.)
    /// 3. Store the parsed config internally
    ///
    /// # Errors
    ///
    /// Returns `DomainConfigError` if:
    /// - Config file cannot be read
    /// - Config file cannot be parsed
    fn load(&mut self, config: &dyn ConfigProvider) -> Result<(), DomainConfigError>;

    /// Validate the loaded configuration.
    ///
    /// This method should:
    /// 1. Check semantic validity (not just syntax)
    /// 2. Verify referenced resources exist
    /// 3. Check for conflicts or inconsistencies
    ///
    /// Must be called after `load()`.
    ///
    /// # Returns
    ///
    /// A `ValidationReport` containing any errors or warnings.
    fn validate(&self) -> Result<ValidationReport, DomainConfigError>;

    /// Dependencies on other domains.
    ///
    /// Return domain IDs that must be loaded before this one.
    /// Default: no dependencies.
    fn dependencies(&self) -> &[&'static str] {
        &[]
    }

    /// Priority for load order (lower = earlier).
    ///
    /// Default: 100. Core domains use 1-10, extensions use 100+.
    fn priority(&self) -> u32 {
        100
    }
}

/// Registry of domain config validators.
///
/// Used by `StartupValidator` to collect and run all domain validators.
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

    /// Register a domain validator.
    pub fn register(&mut self, validator: Box<dyn DomainConfig>) {
        self.validators.push(validator);
    }

    /// Get validators sorted by priority and dependencies.
    pub fn validators_sorted(&self) -> Vec<&dyn DomainConfig> {
        let mut validators: Vec<_> = self.validators.iter().map(AsRef::as_ref).collect();
        validators.sort_by_key(|v| v.priority());
        validators
    }

    /// Get mutable validators for loading.
    pub fn validators_mut(&mut self) -> impl Iterator<Item = &mut Box<dyn DomainConfig>> {
        // Sort by priority first
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

// Tests are in crates/shared/traits-tests/ per architecture policy
