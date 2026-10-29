//! `cloud doctor`'s restore checks: whether the target database's schema is
//! newer than this binary, and whether uploaded files sit outside the
//! database's point-in-time recovery.
//!
//! Migrations only move forward. A database whose ledger records a
//! migration numbered past every one this binary declares was either
//! restored from a newer release or is being served by a rolled-back image;
//! either way this binary cannot own its schema, so that is a blocking
//! failure. Files on a node-local `storage.backend` are not in the database,
//! so a split-role deployment that restores its database alone loses them;
//! that is a warning.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::time::Duration;

use systemprompt_database::{Database, MigrationService, validate_table_exists};
use systemprompt_extension::ExtensionRegistry;
use systemprompt_manifest::Profile;
use systemprompt_manifest::profile::StorageBackend;

use super::CheckResult;

const SCHEMA: &str = "schema-version";
const STORAGE: &str = "storage-backup";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub const FORWARD_ONLY_HINT: &str =
    "migrations are forward-only: restore the database, not an older image";

pub fn check_storage_backup(profile: &Profile) -> CheckResult {
    let role = profile.server.role;
    if role.is_single_node() {
        return CheckResult::pass(STORAGE, "single-node role; files share the node's backup");
    }
    if profile.storage.backend == StorageBackend::Local {
        return CheckResult::warn(
            STORAGE,
            format!(
                "server.role is '{role}' and storage.backend is 'local': uploaded files are \
                 outside database point-in-time recovery; use storage.backend 's3' or 'gcs' so \
                 a database restore keeps its files"
            ),
        );
    }
    CheckResult::pass(STORAGE, "files are on object storage")
}

#[must_use]
pub fn schema_ahead_result(ahead: &[(String, Vec<u32>)]) -> CheckResult {
    if ahead.is_empty() {
        return CheckResult::pass(SCHEMA, "database schema is not ahead of this binary");
    }
    let listed: Vec<String> = ahead
        .iter()
        .map(|(extension, versions)| format!("{extension} {versions:?}"))
        .collect();
    CheckResult::fail(
        SCHEMA,
        format!(
            "database holds migrations newer than this binary ({}) — {FORWARD_ONLY_HINT}",
            listed.join(", ")
        ),
    )
}

pub(super) async fn check_schema_not_ahead(secrets: &HashMap<String, String>) -> CheckResult {
    let Some(url) = secrets
        .get("external_database_url")
        .or_else(|| secrets.get("database_url"))
    else {
        return CheckResult::warn(
            SCHEMA,
            "no database URL, so the schema version was not checked",
        );
    };
    let db = match tokio::time::timeout(CONNECT_TIMEOUT, Database::new_postgres(url)).await {
        Ok(Ok(db)) => db,
        Ok(Err(error)) => {
            return CheckResult::warn(
                SCHEMA,
                format!("database unreachable ({error}), schema version not checked"),
            );
        },
        Err(_) => {
            return CheckResult::warn(
                SCHEMA,
                "database did not answer within 10s, schema version not checked",
            );
        },
    };
    match validate_table_exists(&db, "extension_migrations").await {
        Ok(true) => {},
        Ok(false) => return CheckResult::pass(SCHEMA, "fresh database; no migrations recorded"),
        Err(error) => {
            return CheckResult::warn(
                SCHEMA,
                format!("migration ledger unreadable ({error}), schema version not checked"),
            );
        },
    }
    let registry = match ExtensionRegistry::discover() {
        Ok(registry) => registry,
        Err(error) => {
            return CheckResult::warn(
                SCHEMA,
                format!("extensions could not be discovered ({error}), schema not checked"),
            );
        },
    };
    let migrations = MigrationService::new(&db);
    let mut ahead = Vec::new();
    for extension in registry.schema_extensions() {
        match migrations.status(extension.as_ref()).await {
            Ok(status) if !status.ahead.is_empty() => {
                ahead.push((status.extension_id.as_str().to_owned(), status.ahead));
            },
            Ok(_) => {},
            Err(error) => {
                return CheckResult::warn(
                    SCHEMA,
                    format!(
                        "migration status for {} unreadable ({error}), schema not checked",
                        extension.id()
                    ),
                );
            },
        }
    }
    schema_ahead_result(&ahead)
}
