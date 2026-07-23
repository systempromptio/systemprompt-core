//! Extension registration for the database crate's own schema.
//!
//! Every systemprompt extension that owns DDL registers itself through the
//! `inventory` framework. The database crate registers its own bookkeeping
//! tables (`extension_migrations`) and shared SQL helper functions so that
//! they install before any downstream extension runs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

#[derive(Debug, Clone, Copy, Default)]
pub struct DatabaseExtension;

impl Extension for DatabaseExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "database",
            name: "Database",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![
            SchemaDefinition::new(
                "extension_migrations",
                include_str!("../schema/extension_migrations.sql"),
            ),
            SchemaDefinition::new("functions", include_str!("../schema/functions.sql")),
        ]
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec![]
    }

    fn priority(&self) -> u32 {
        // Why: No extension declares `database` as a dependency, so install order is
        // decided purely by priority among roots; 0 keeps the shared helper
        // functions and `extension_migrations` ahead of every other extension.
        0
    }
}

register_extension!(DatabaseExtension);
