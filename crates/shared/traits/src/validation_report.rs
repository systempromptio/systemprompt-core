//! Unified validation report types for startup validation.
//!
//! These types provide a consistent structure for reporting validation
//! errors and warnings across all domains and extensions.

use std::path::PathBuf;

/// A single validation error with actionable information.
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// The field or config key that failed validation
    pub field: String,

    /// Human-readable error message
    pub message: String,

    /// Optional path to the file/directory that caused the error
    pub path: Option<PathBuf>,

    /// Optional suggestion for how to fix the error
    pub suggestion: Option<String>,
}

impl ValidationError {
    #[must_use]
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
            path: None,
            suggestion: None,
        }
    }

    #[must_use]
    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = Some(path.into());
        self
    }

    #[must_use]
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}\n  {}", self.field, self.message)?;
        if let Some(ref path) = self.path {
            write!(f, "\n  Path: {}", path.display())?;
        }
        if let Some(ref suggestion) = self.suggestion {
            write!(f, "\n  To fix: {}", suggestion)?;
        }
        Ok(())
    }
}

/// A validation warning (non-fatal).
#[derive(Debug, Clone)]
pub struct ValidationWarning {
    /// The field or config key that triggered the warning
    pub field: String,

    /// Human-readable warning message
    pub message: String,

    /// Optional suggestion for how to address the warning
    pub suggestion: Option<String>,
}

impl ValidationWarning {
    #[must_use]
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
            suggestion: None,
        }
    }

    #[must_use]
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

/// Validation report for a single domain or component.
#[derive(Debug, Clone, Default)]
pub struct ValidationReport {
    /// Domain identifier (e.g., "paths", "web", "content", "mcp")
    pub domain: String,

    /// List of errors (fatal)
    pub errors: Vec<ValidationError>,

    /// List of warnings (non-fatal)
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationReport {
    #[must_use]
    pub fn new(domain: impl Into<String>) -> Self {
        Self {
            domain: domain.into(),
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn add_error(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    pub fn add_warning(&mut self, warning: ValidationWarning) {
        self.warnings.push(warning);
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    pub fn is_clean(&self) -> bool {
        self.errors.is_empty() && self.warnings.is_empty()
    }

    /// Merge another report into this one.
    pub fn merge(&mut self, other: Self) {
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
    }
}

/// Complete startup validation report.
#[derive(Debug, Clone, Default)]
pub struct StartupValidationReport {
    /// Path to the profile that was validated
    pub profile_path: Option<PathBuf>,

    /// Reports from each domain
    pub domains: Vec<ValidationReport>,

    /// Reports from extensions
    pub extensions: Vec<ValidationReport>,
}

impl StartupValidationReport {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_profile_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.profile_path = Some(path.into());
        self
    }

    pub fn add_domain(&mut self, report: ValidationReport) {
        self.domains.push(report);
    }

    pub fn add_extension(&mut self, report: ValidationReport) {
        self.extensions.push(report);
    }

    pub fn has_errors(&self) -> bool {
        self.domains.iter().any(ValidationReport::has_errors)
            || self.extensions.iter().any(ValidationReport::has_errors)
    }

    pub fn has_warnings(&self) -> bool {
        self.domains.iter().any(ValidationReport::has_warnings)
            || self.extensions.iter().any(ValidationReport::has_warnings)
    }

    pub fn error_count(&self) -> usize {
        self.domains.iter().map(|r| r.errors.len()).sum::<usize>()
            + self
                .extensions
                .iter()
                .map(|r| r.errors.len())
                .sum::<usize>()
    }

    pub fn warning_count(&self) -> usize {
        self.domains.iter().map(|r| r.warnings.len()).sum::<usize>()
            + self
                .extensions
                .iter()
                .map(|r| r.warnings.len())
                .sum::<usize>()
    }
}

impl std::fmt::Display for StartupValidationReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} error(s), {} warning(s)",
            self.error_count(),
            self.warning_count()
        )
    }
}

/// Error type for startup validation failures.
#[derive(Debug, thiserror::Error)]
#[error("Startup validation failed with {0}")]
pub struct StartupValidationError(pub StartupValidationReport);

impl From<StartupValidationReport> for StartupValidationError {
    fn from(report: StartupValidationReport) -> Self {
        Self(report)
    }
}
