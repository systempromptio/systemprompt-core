#[derive(Debug, Clone, Default)]
pub struct IngestionReport {
    pub files_found: usize,
    pub files_processed: usize,
    pub errors: Vec<String>,
}

impl IngestionReport {
    pub fn new() -> Self {
        Self::default()
    }

    pub const fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub const fn successful_count(&self) -> usize {
        self.files_processed.saturating_sub(self.errors.len())
    }

    pub const fn failed_count(&self) -> usize {
        self.errors.len()
    }
}
