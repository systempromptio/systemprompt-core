//! `cloud doctor`'s Git credential check: every registered Git source that
//! names a credential label has its secret under the host-bound name the
//! runtime resolves.
//!
//! A secret still stored under the bare label fails the check and names the
//! migrate command; the runtime never falls back to the bare label. A label
//! with no secret under either name is a warning.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::hash::BuildHasher;
use std::sync::Arc;
use std::time::Duration;

use sqlx::postgres::PgPoolOptions;
use systemprompt_database::Database;
use systemprompt_marketplace::managed::{GitCredentialSource, ManagedRepository};
use systemprompt_runtime::managed::git_sources::{
    MIGRATE_GIT_CREDENTIALS_COMMAND, git_credential_key,
};

use super::CheckResult;

const NAME: &str = "git-credentials";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) async fn check_git_credentials(secrets: &HashMap<String, String>) -> CheckResult {
    let Some(url) = secrets
        .get("external_database_url")
        .or_else(|| secrets.get("database_url"))
    else {
        return CheckResult::warn(NAME, "no database URL, so Git sources were not checked");
    };
    let pool = match PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(CONNECT_TIMEOUT)
        .connect(url)
        .await
    {
        Ok(pool) => Arc::new(pool),
        Err(error) => {
            return CheckResult::warn(
                NAME,
                format!("database unreachable ({error}), Git sources not checked"),
            );
        },
    };
    let db = Arc::new(Database::from_pools(Arc::clone(&pool), None));
    let sources = ManagedRepository::new(&db)
        .git_sources_with_credentials()
        .await;
    pool.close().await;
    match sources {
        Ok(sources) => classify_git_credentials(secrets, &sources),
        Err(error) => CheckResult::warn(NAME, format!("Git sources could not be listed ({error})")),
    }
}

pub fn classify_git_credentials<S: BuildHasher>(
    secrets: &HashMap<String, String, S>,
    sources: &[GitCredentialSource],
) -> CheckResult {
    let present = |name: &str| {
        [name.to_owned(), name.to_uppercase(), name.to_lowercase()]
            .iter()
            .any(|key| secrets.get(key).is_some_and(|value| !value.is_empty()))
    };
    let mut unmigrated = Vec::new();
    let mut missing = Vec::new();
    for source in sources {
        let label = source.credential_reference.as_str();
        match git_credential_key(&source.repository, label) {
            Some(key) if present(&key) => {},
            Some(key) if present(label) => {
                unmigrated.push(format!("{} (label {label} -> {key})", source.name));
            },
            _ => missing.push(format!("{} (label {label})", source.name)),
        }
    }
    if !unmigrated.is_empty() {
        return CheckResult::fail(
            NAME,
            format!(
                "credential stored under its bare label, which the runtime does not use: {}; \
                 run `systemprompt {MIGRATE_GIT_CREDENTIALS_COMMAND} --apply`",
                unmigrated.join(", ")
            ),
        );
    }
    if !missing.is_empty() {
        return CheckResult::warn(
            NAME,
            format!("no credential secret found for: {}", missing.join(", ")),
        );
    }
    CheckResult::pass(
        NAME,
        format!("{} Git source credential(s) resolve", sources.len()),
    )
}
