//! Schema migration value type.

/// Expand to the body of
/// [`Extension::migrations`](crate::Extension::migrations), generated from
/// `schema/migrations/NNN_<name>.sql` files by
/// [`build::emit_migrations`](crate::build::emit_migrations).
///
/// The using crate must have a `build.rs` that calls
/// `systemprompt_extension::build::emit_migrations()`.
///
/// ```rust,ignore
/// fn migrations(&self) -> Vec<Migration> {
///     extension_migrations!()
/// }
/// ```
#[macro_export]
macro_rules! extension_migrations {
    () => {
        include!(concat!(env!("OUT_DIR"), "/migrations.rs"))
    };
}

#[derive(Debug, Clone)]
pub struct Migration {
    pub version: u32,
    pub name: String,
    pub sql: &'static str,
    pub down: Option<&'static str>,
    pub no_transaction: bool,
}

impl Migration {
    #[must_use]
    pub fn new(version: u32, name: impl Into<String>, sql: &'static str) -> Self {
        Self {
            version,
            name: name.into(),
            sql,
            down: None,
            no_transaction: false,
        }
    }

    #[must_use]
    pub fn with_down(
        version: u32,
        name: impl Into<String>,
        up_sql: &'static str,
        down_sql: &'static str,
    ) -> Self {
        Self {
            version,
            name: name.into(),
            sql: up_sql,
            down: Some(down_sql),
            no_transaction: false,
        }
    }

    /// Constructor for migrations that must run outside an implicit
    /// transaction — e.g. `CREATE INDEX CONCURRENTLY`, which Postgres rejects
    /// inside a transaction block.
    #[must_use]
    pub fn new_no_transaction(version: u32, name: impl Into<String>, sql: &'static str) -> Self {
        Self {
            version,
            name: name.into(),
            sql,
            down: None,
            no_transaction: true,
        }
    }

    #[must_use]
    pub fn checksum(&self) -> String {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.sql.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }
}
