//! Unified validation report types used during startup.

use std::path::PathBuf;

/// Single validation failure with optional path and remediation hint.
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// Field that failed validation.
    pub field: String,
    /// Human-readable description of the failure.
    pub message: String,
    /// Optional filesystem path that produced the failure.
    pub path: Option<PathBuf>,
    /// Optional suggestion for resolving the failure.
    pub suggestion: Option<String>,
}

impl ValidationError {
    /// Construct a [`ValidationError`] for `field`.
    #[must_use]
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
            path: None,
            suggestion: None,
        }
    }

    /// Attach a filesystem path for context.
    #[must_use]
    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// Attach a human-readable remediation hint.
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

/// Non-blocking validation warning.
#[derive(Debug, Clone)]
pub struct ValidationWarning {
    /// Field the warning relates to.
    pub field: String,
    /// Human-readable description.
    pub message: String,
    /// Optional remediation hint.
    pub suggestion: Option<String>,
}

impl ValidationWarning {
    /// Construct a [`ValidationWarning`] for `field`.
    #[must_use]
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
            suggestion: None,
        }
    }

    /// Attach a remediation hint.
    #[must_use]
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

/// Collection of validation findings for a single domain or extension.
#[derive(Debug, Clone, Default)]
pub struct ValidationReport {
    /// Domain or extension name the report belongs to.
    pub domain: String,
    /// Blocking errors recorded by the validator.
    pub errors: Vec<ValidationError>,
    /// Non-blocking warnings recorded by the validator.
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationReport {
    /// Construct an empty report tagged with `domain`.
    #[must_use]
    pub fn new(domain: impl Into<String>) -> Self {
        Self {
            domain: domain.into(),
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Append an error to the report.
    pub fn add_error(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    /// Append a warning to the report.
    pub fn add_warning(&mut self, warning: ValidationWarning) {
        self.warnings.push(warning);
    }

    /// Report whether any errors were recorded.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Report whether any warnings were recorded.
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    /// Report whether the report is fully clean (no errors or warnings).
    pub fn is_clean(&self) -> bool {
        self.errors.is_empty() && self.warnings.is_empty()
    }

    /// Move all errors and warnings from `other` into this report.
    pub fn merge(&mut self, other: Self) {
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
    }
}

/// Aggregated startup validation result across every domain and extension.
#[derive(Debug, Clone, Default)]
pub struct StartupValidationReport {
    /// Path of the active profile, if known.
    pub profile_path: Option<PathBuf>,
    /// Per-domain reports.
    pub domains: Vec<ValidationReport>,
    /// Per-extension reports.
    pub extensions: Vec<ValidationReport>,
}

impl StartupValidationReport {
    /// Construct an empty startup report.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach the active profile path.
    #[must_use]
    pub fn with_profile_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.profile_path = Some(path.into());
        self
    }

    /// Append a domain report.
    pub fn add_domain(&mut self, report: ValidationReport) {
        self.domains.push(report);
    }

    /// Append an extension report.
    pub fn add_extension(&mut self, report: ValidationReport) {
        self.extensions.push(report);
    }

    /// Report whether any sub-report has errors.
    pub fn has_errors(&self) -> bool {
        self.domains.iter().any(ValidationReport::has_errors)
            || self.extensions.iter().any(ValidationReport::has_errors)
    }

    /// Report whether any sub-report has warnings.
    pub fn has_warnings(&self) -> bool {
        self.domains.iter().any(ValidationReport::has_warnings)
            || self.extensions.iter().any(ValidationReport::has_warnings)
    }

    /// Total error count across every sub-report.
    pub fn error_count(&self) -> usize {
        self.domains.iter().map(|r| r.errors.len()).sum::<usize>()
            + self
                .extensions
                .iter()
                .map(|r| r.errors.len())
                .sum::<usize>()
    }

    /// Total warning count across every sub-report.
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

/// Wrapper that turns a [`StartupValidationReport`] into a `std::error::Error`.
#[derive(Debug, thiserror::Error)]
#[error("Startup validation failed with {0}")]
pub struct StartupValidationError(pub StartupValidationReport);

impl From<StartupValidationReport> for StartupValidationError {
    fn from(report: StartupValidationReport) -> Self {
        Self(report)
    }
}
