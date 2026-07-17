//! Owner resolution for scheduled jobs.
//!
//! A job with no explicit owner runs as the profile `system_admin`; a job whose
//! explicit owner does not resolve to an active user is dropped from the
//! schedule (collected as a [`SkippedJob`]) and, on the live `start` pass,
//! recorded as an `ERROR` in the `logs` table.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_identifiers::{SessionId, TraceId, UserId};
use systemprompt_logging::{LogActor, LogEntry, LogLevel, LoggingRepository};
use systemprompt_models::auth::UserStatus;
use systemprompt_users::UserRepository;
use tracing::{debug, warn};

use super::SchedulerService;
use crate::error::SchedulerResult;
use crate::models::SkippedJob;

pub(super) struct ResolvedOwners {
    map: HashMap<String, UserId>,
    skipped: Vec<SkippedJob>,
}

impl ResolvedOwners {
    pub(super) const fn owner_map(&self) -> &HashMap<String, UserId> {
        &self.map
    }

    pub(super) fn skipped_names(&self) -> impl Iterator<Item = &str> {
        self.skipped.iter().map(|job| job.job_name.as_str())
    }

    pub(super) fn into_degraded(self) -> Vec<SkippedJob> {
        self.skipped
    }
}

impl SchedulerService {
    pub(super) async fn resolve_owners(&self, emit_logs: bool) -> SchedulerResult<ResolvedOwners> {
        let users = UserRepository::new(&self.db_pool)?;
        let system_admin_id = self.app_context.system_admin().id();
        let mut map = HashMap::with_capacity(self.config.jobs.len());
        let mut skipped = Vec::new();
        for job in self.config.jobs.iter().filter(|j| j.enabled) {
            let Some(owner) = &job.owner else {
                map.insert(job.name.clone(), system_admin_id.clone());
                continue;
            };
            let active = users
                .find_by_name(owner.as_str())
                .await?
                .filter(|u| u.status.as_deref() == Some(UserStatus::Active.as_str()));
            if let Some(user) = active {
                debug!(job_name = %job.name, owner = %user.id, "resolved job owner");
                map.insert(job.name.clone(), user.id);
            } else {
                warn!(job_name = %job.name, owner = %owner, "job owner is not an active user, skipping job");
                skipped.push(SkippedJob {
                    job_name: job.name.clone(),
                    owner: owner.as_str().to_owned(),
                    reason: "configured owner is not an active user".to_owned(),
                });
            }
        }
        if emit_logs && !skipped.is_empty() {
            self.persist_skipped_owner_errors(&skipped).await;
        }
        Ok(ResolvedOwners { map, skipped })
    }

    async fn persist_skipped_owner_errors(&self, skipped: &[SkippedJob]) {
        let repository = match LoggingRepository::new(&self.db_pool) {
            Ok(repository) => repository.with_database(true),
            Err(error) => {
                warn!(error = %error, "could not open logging repository to record skipped scheduler jobs");
                return;
            },
        };
        let system_admin_id = self.app_context.system_admin().id().clone();
        for job in skipped {
            let actor = LogActor::new(
                system_admin_id.clone(),
                SessionId::system(),
                TraceId::generate(),
            );
            let entry = LogEntry::new(
                LogLevel::Error,
                "scheduler",
                format!(
                    "Job '{}' declares owner '{}' which is not an active user; job skipped. \
                     Remove the owner to default to the system admin, or set it to an active user.",
                    job.job_name, job.owner
                ),
                actor,
            );
            if let Err(error) = repository.log(entry).await {
                warn!(error = %error, job_name = %job.job_name, "failed to persist scheduler owner error to logs");
            }
        }
    }
}
