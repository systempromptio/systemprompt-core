//! Every admitted member's scheduled work, stamped with its owner in
//! composition order: jobs, retention purges, sweeps and holds, and the
//! scopes a job run executes inside.
//!
//! The kernel builds one [`AdmittedSchedule`] from the admitted composition
//! and seeds it into state; the scheduler, its jobs and the `infra jobs`
//! commands read it. A job name has exactly one owner: two admitted members
//! contributing the same name refuse the build with [`DuplicateJobOwner`]
//! naming both, before any schedule row is written. Each job keeps the
//! standing the facade stamped on its owner ([`AdmittedSchedule::owner_of`]),
//! so a privilege bound to a job is decided by its admitted owner, never by
//! its name. A disabled member is never admitted, so its jobs are
//! unknown names here and its retention work and scopes never run.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::fmt;
use std::pin::Pin;
use std::sync::Arc;

use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::{Job, JobResult, ProviderResult};
use systemprompt_traits::{JobRunScope, RetentionPurge, RetentionSweep};

use crate::admitted::BuiltState;
use crate::contribute::StandingTag;
use crate::retention::{RetentionDecl, RetentionHold, RetentionHolds};

/// Two admitted members contributed a job of the same name.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "job '{job}' is contributed by both '{first}' and '{second}'; every job name has exactly one \
     owner"
)]
pub struct DuplicateJobOwner {
    pub job: &'static str,
    pub first: ExtensionId,
    pub second: ExtensionId,
}

impl DuplicateJobOwner {
    pub fn refuse<'a>(
        jobs: impl IntoIterator<Item = (&'a ExtensionId, &'static str)>,
    ) -> Result<(), Self> {
        let mut owners: HashMap<&'static str, &ExtensionId> = HashMap::new();
        for (owner, job) in jobs {
            if let Some(first) = owners.insert(job, owner) {
                return Err(Self {
                    job,
                    first: first.clone(),
                    second: owner.clone(),
                });
            }
        }
        Ok(())
    }
}

type JobRun<'a> = Pin<Box<dyn Future<Output = ProviderResult<JobResult>> + Send + 'a>>;

/// The owner-stamped jobs, retention work and job-run scopes of an admitted
/// composition.
#[derive(Clone, Default)]
pub struct AdmittedSchedule {
    jobs: Vec<(ExtensionId, Arc<dyn Job>)>,
    owners: HashMap<&'static str, (ExtensionId, StandingTag)>,
    purges: Vec<(ExtensionId, Arc<dyn RetentionPurge>)>,
    sweeps: Vec<(ExtensionId, Arc<dyn RetentionSweep>)>,
    holds: Vec<(ExtensionId, Arc<dyn RetentionHold>)>,
    scopes: Vec<(ExtensionId, Arc<dyn JobRunScope>)>,
}

impl fmt::Debug for AdmittedSchedule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AdmittedSchedule")
            .field("jobs", &self.job_names().collect::<Vec<_>>())
            .field(
                "purges",
                &self
                    .purges
                    .iter()
                    .map(|(_, p)| p.table())
                    .collect::<Vec<_>>(),
            )
            .field(
                "sweeps",
                &self
                    .sweeps
                    .iter()
                    .map(|(_, s)| s.name())
                    .collect::<Vec<_>>(),
            )
            .field(
                "holds",
                &self.holds.iter().map(|(_, h)| h.name()).collect::<Vec<_>>(),
            )
            .field(
                "scopes",
                &self
                    .scopes
                    .iter()
                    .map(|(_, s)| s.name())
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl AdmittedSchedule {
    pub fn of(built: &BuiltState) -> Result<Self, DuplicateJobOwner> {
        DuplicateJobOwner::refuse(built.jobs().map(|(owner, job)| (owner, job.job().name())))?;
        let mut schedule = Self::default();
        for member in built.members() {
            let owner = &member.owner;
            for contributed in member.sink.jobs() {
                let job = contributed.job();
                schedule
                    .owners
                    .insert(job.name(), (owner.clone(), member.standing));
                schedule.jobs.push((owner.clone(), Arc::clone(job)));
            }
            for decl in member.sink.retention() {
                match decl {
                    RetentionDecl::Purge(purge) => {
                        schedule.purges.push((owner.clone(), Arc::clone(purge)));
                    },
                    RetentionDecl::Sweep(sweep) => {
                        schedule.sweeps.push((owner.clone(), Arc::clone(sweep)));
                    },
                    RetentionDecl::Hold(hold) => {
                        schedule.holds.push((owner.clone(), Arc::clone(hold)));
                    },
                }
            }
            schedule.scopes.extend(
                member
                    .sink
                    .job_run_scopes()
                    .iter()
                    .map(|scope| (owner.clone(), Arc::clone(scope))),
            );
        }
        Ok(schedule)
    }

    #[must_use]
    pub fn owner_of(&self, name: &str) -> Option<(&ExtensionId, StandingTag)> {
        self.owners
            .get(name)
            .map(|(owner, standing)| (owner, *standing))
    }

    #[must_use]
    pub fn jobs(&self) -> &[(ExtensionId, Arc<dyn Job>)] {
        &self.jobs
    }

    pub fn job_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.jobs.iter().map(|(_, job)| job.name())
    }

    #[must_use]
    pub fn job(&self, name: &str) -> Option<&(ExtensionId, Arc<dyn Job>)> {
        self.jobs.iter().find(|(_, job)| job.name() == name)
    }

    #[must_use]
    pub fn contains_job(&self, name: &str) -> bool {
        self.job(name).is_some()
    }

    #[must_use]
    pub fn purges(&self) -> &[(ExtensionId, Arc<dyn RetentionPurge>)] {
        &self.purges
    }

    #[must_use]
    pub fn sweeps(&self) -> &[(ExtensionId, Arc<dyn RetentionSweep>)] {
        &self.sweeps
    }

    #[must_use]
    pub fn holds(&self) -> RetentionHolds {
        RetentionHolds::new(
            self.holds
                .iter()
                .map(|(_, hold)| Arc::clone(hold))
                .collect(),
        )
    }

    #[must_use]
    pub fn job_run_scopes(&self) -> &[(ExtensionId, Arc<dyn JobRunScope>)] {
        &self.scopes
    }

    pub fn run_in_scopes<'a, F>(&self, job: &'static str, run: F) -> JobRun<'a>
    where
        F: Future<Output = ProviderResult<JobResult>> + Send + 'a,
    {
        let run: JobRun<'a> = Box::pin(run);
        self.scopes
            .iter()
            .fold(run, |inner, (_, scope)| scope.scope(job, inner))
    }
}
