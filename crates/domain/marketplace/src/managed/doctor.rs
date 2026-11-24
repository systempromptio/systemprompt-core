//! The `managed_resources` member's `cloud doctor` check: every registered
//! Git source that names a credential label has its secret under the
//! host-bound name the runtime resolves.
//!
//! A secret still stored under the bare label fails the check and names the
//! migrate command; the runtime never falls back to the bare label. A label
//! with no secret under either name is a warning, and so is a run where the
//! doctor's read-only probe cannot list the sources. The check reads only
//! whether a secret is present, never its value, and the source list the
//! doctor's probe answers; it holds no database handle. A probe read failure
//! is logged with its cause and reported with a fixed detail, so no database
//! error text reaches the report.
//!
//! [`GitSourceProbes`] is the member's doctor probe source: over the doctor's
//! read-only pool it answers the Git source list from the managed sources
//! table, so the doctor reads no marketplace repository itself.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;
use systemprompt_database::Database;
use systemprompt_extension::DoctorProbeSource;
use systemprompt_traits::{
    DoctorCheck, DoctorContext, DoctorOutcome, DoctorProbeError, DoctorProbes, GitCredentialSource,
    SecretPresence,
};

use super::ManagedRepository;
use super::git_sources::{MIGRATE_GIT_CREDENTIALS_COMMAND, git_credential_key};

/// The Git source credential check `cloud doctor` runs for this member.
#[derive(Debug, Clone, Copy, Default)]
pub struct GitCredentialCheck;

#[async_trait]
impl DoctorCheck for GitCredentialCheck {
    fn name(&self) -> &'static str {
        "git-credentials"
    }

    async fn check(&self, ctx: DoctorContext<'_>) -> DoctorOutcome {
        match ctx.probes.git_credential_sources().await {
            Ok(sources) => classify_git_credentials(ctx.secrets, &sources),
            Err(DoctorProbeError::Unreachable) => {
                DoctorOutcome::warn("no reachable database, so Git sources were not checked")
            },
            Err(error) => {
                tracing::warn!(error = ?error, "doctor: Git source probe read failed");
                DoctorOutcome::warn("Git sources could not be listed")
            },
        }
    }
}

/// The doctor probe source this member contributes.
#[derive(Debug, Clone, Copy, Default)]
pub struct GitSourceProbes;

impl DoctorProbeSource for GitSourceProbes {
    fn probes(&self, pool: Arc<PgPool>) -> Box<dyn DoctorProbes> {
        Box::new(ManagedProbes {
            managed: ManagedRepository::new(&Arc::new(Database::from_pools(pool, None))),
        })
    }
}

struct ManagedProbes {
    managed: ManagedRepository,
}

#[async_trait]
impl DoctorProbes for ManagedProbes {
    async fn git_credential_sources(&self) -> Result<Vec<GitCredentialSource>, DoctorProbeError> {
        self.managed
            .git_sources_with_credentials()
            .await
            .map_err(|error| DoctorProbeError::Read(Box::new(error)))
    }
}

pub fn classify_git_credentials(
    secrets: &dyn SecretPresence,
    sources: &[GitCredentialSource],
) -> DoctorOutcome {
    let present = |name: &str| {
        [name.to_owned(), name.to_uppercase(), name.to_lowercase()]
            .iter()
            .any(|key| secrets.is_present(key))
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
        return DoctorOutcome::fail(format!(
            "credential stored under its bare label, which the runtime does not use: {}; run \
             `systemprompt {MIGRATE_GIT_CREDENTIALS_COMMAND} --apply`",
            unmigrated.join(", ")
        ));
    }
    if !missing.is_empty() {
        return DoctorOutcome::warn(format!(
            "no credential secret found for: {}",
            missing.join(", ")
        ));
    }
    DoctorOutcome::pass(format!(
        "{} Git source credential(s) resolve",
        sources.len()
    ))
}
