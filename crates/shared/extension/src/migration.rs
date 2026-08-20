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
