//! Schema migration value type.

/// Single SQL migration owned by an extension.
#[derive(Debug, Clone)]
pub struct Migration {
    /// Monotonic version number used to order migrations within an
    /// extension.
    pub version: u32,
    /// Human-readable migration name (typically the file stem).
    pub name: String,
    /// SQL body, embedded at compile time.
    pub sql: &'static str,
}

impl Migration {
    /// Constructs a new migration descriptor.
    #[must_use]
    pub fn new(version: u32, name: impl Into<String>, sql: &'static str) -> Self {
        Self {
            version,
            name: name.into(),
            sql,
        }
    }

    /// Returns a stable hex digest of the SQL body.
    ///
    /// Used by the migration runner to detect drift between recorded and
    /// embedded migration sources.
    #[must_use]
    pub fn checksum(&self) -> String {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.sql.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }
}
