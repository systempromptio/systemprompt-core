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
            SchemaDefinition::new(
                "extension_migration_compat",
                include_str!("../schema/extension_migration_compat.sql"),
            ),
            SchemaDefinition::new("services", include_str!("../schema/services.sql"))
                .with_required_columns(vec!["name".into(), "module_name".into(), "status".into()]),
            SchemaDefinition::new(
                "projection_backlog",
                include_str!("../schema/projection_backlog.sql"),
            )
            .with_required_columns(vec![
                "projection".into(),
                "key".into(),
                "attempts".into(),
            ]),
            SchemaDefinition::new("trace_facts", include_str!("../schema/trace_facts.sql"))
                .with_required_columns(vec!["trace_id".into(), "first_at".into(), "status".into()]),
            SchemaDefinition::new(
                "instance_leases",
                include_str!("../schema/instance_leases.sql"),
            )
            .with_required_columns(vec![
                "lease_key".into(),
                "owner_instance_id".into(),
                "expires_at".into(),
            ]),
            SchemaDefinition::new(
                "instance_endpoints",
                include_str!("../schema/instance_endpoints.sql"),
            )
            .with_required_columns(vec![
                "instance_id".into(),
                "advertise_url".into(),
                "heartbeat_at".into(),
            ]),
            SchemaDefinition::new(
                "instance_presence",
                include_str!("../schema/instance_presence.sql"),
            )
            .with_required_columns(vec![
                "instance_id".into(),
                "role".into(),
                "heartbeat_at".into(),
            ]),
            SchemaDefinition::new(
                "release_stamps",
                include_str!("../schema/release_stamps.sql"),
            )
            .with_required_columns(vec!["name".into(), "version".into()]),
            SchemaDefinition::new(
                "config_version",
                include_str!("../schema/config_version.sql"),
            )
            .with_required_columns(vec!["id".into(), "version".into(), "plane".into()]),
            SchemaDefinition::new(
                "config_plane_applied",
                include_str!("../schema/config_plane_applied.sql"),
            )
            .with_required_columns(vec!["plane".into(), "applied_at".into()]),
            SchemaDefinition::sql_only(include_str!("../schema/functions.sql")),
            SchemaDefinition::sql_only(include_str!("../schema/config_change_triggers.sql")),
        ]
    }

    fn migrations(&self) -> Vec<Migration> {
        extension_migrations!()
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec![]
    }

    fn priority(&self) -> u32 {
        0
    }
}

register_extension!(DatabaseExtension);
