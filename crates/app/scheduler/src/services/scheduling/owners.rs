//! Owner resolution for scheduled jobs.
//!
//! A job with no explicit owner runs as the profile `system_admin`; a job whose
//! explicit owner does not resolve to an active user (or that no admitted
//! user directory can resolve) is dropped from the schedule (collected as a
//! [`SkippedJob`]) and, on the live `start` pass, recorded as an `ERROR` in the
//! `logs` table.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_identifiers::{JobName, SessionId, TraceId, UserId};
use systemprompt_logging::{LogActor, LogEntry, LogLevel};
use tracing::{debug, warn};

use super::SchedulerService;
use crate::error::SchedulerResult;
use crate::models::SkippedJob;

pub(super) struct ResolvedOwners {
    map: HashMap<JobName, UserId>,
    skipped: Vec<SkippedJob>,
}

impl ResolvedOwners {
    pub(super) const fn owner_map(&self) -> &HashMap<JobName, UserId> {
        &self.map
    }

    pub(super) fn skipped_names(&self) -> impl Iterator<Item = &JobName> {
        self.skipped.iter().map(|job| &job.job_name)
    }

    pub(super) fn into_degraded(self) -> Vec<SkippedJob> {
        self.skipped
    }
}

impl SchedulerService {
    pub(super) async fn resolve_owners(&self, emit_logs: bool) -> SchedulerResult<ResolvedOwners> {
        let system_admin_id = &self.system_admin_id;
        let mut map = HashMap::with_capacity(self.config.jobs.len());
        let mut skipped = Vec::new();
        for job in self.config.jobs.iter().filter(|j| j.enabled) {
            let Some(owner) = &job.owner else {
                map.insert(job.name.clone(), system_admin_id.clone());
                continue;
            };
            let active = match &self.users {
                Some(users) => users
                    .find_by_name(owner.as_str())
                    .await?
                    .filter(|u| u.is_active),
                None => None,
            };
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
        let repository = &self.logging_repository;
        let system_admin_id = self.system_admin_id.clone();
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
            let entry = match self.log_instance_id.as_ref() {
                Some(instance_id) => entry.with_instance_id(instance_id.clone()),
                None => entry,
            };
            if let Err(error) = repository.log(entry).await {
                warn!(error = %error, job_name = %job.job_name, "failed to persist scheduler owner error to logs");
            }
        }
    }
}
