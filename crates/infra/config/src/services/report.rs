//! Mutable accumulator of errors and warnings produced by
//! [`super::ConfigValidator`].

/// Mutable accumulator of errors and warnings produced by
/// [`super::ConfigValidator`].
#[derive(Debug)]
pub struct ValidationReport {
    /// Hard validation errors. Non-empty implies the run failed.
    pub errors: Vec<String>,
    /// Soft warnings that do not fail the run.
    pub warnings: Vec<String>,
}

impl ValidationReport {
    /// Create an empty report.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Append a new error.
    pub fn add_error(&mut self, error: String) {
        self.errors.push(error);
    }

    /// Append a new warning.
    pub fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }

    /// `true` if no errors have been recorded.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

impl Default for ValidationReport {
    fn default() -> Self {
        Self::new()
    }
}
