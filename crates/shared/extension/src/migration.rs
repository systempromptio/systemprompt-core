//! Schema migration value type.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
    // Why: a spent slot — the migration shipped, its file was later deleted,
    // and established databases still carry its tracking row. Declaring it
    // keeps the number from being refilled. Never executed, never recorded,
    // and never checksummed: the SQL behind the stored checksum is gone.
    pub tombstone: bool,
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
            tombstone: false,
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
            tombstone: false,
        }
    }

    #[must_use]
    pub fn new_no_transaction(version: u32, name: impl Into<String>, sql: &'static str) -> Self {
        Self {
            version,
            name: name.into(),
            sql,
            down: None,
            no_transaction: true,
            tombstone: false,
        }
    }

    // Why: emitted by `build.rs` for a `NNN[-MMM]_<name>.tombstone` file, to
    // declare `version` permanently spent without any SQL. `name` is the
    // deleted migration's own name, so a reused slot can still be reported
    // against the file that originally occupied it.
    #[must_use]
    pub fn tombstone(version: u32, name: impl Into<String>) -> Self {
        Self {
            version,
            name: name.into(),
            sql: "",
            down: None,
            no_transaction: false,
            tombstone: true,
        }
    }

    // Why: never call this on a tombstone — its `sql` is empty, so the hash
    // says nothing about the migration whose checksum the database recorded.
    #[must_use]
    pub fn checksum(&self) -> String {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.sql.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }
}
