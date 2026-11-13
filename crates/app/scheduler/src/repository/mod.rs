//! Persistence layer for the scheduler crate.
//!
//! [`SchedulerRepository`] is the composite façade combining the
//! [`JobRepository`] and the cross-replica job advisory lock; it is the type
//! consumed by [`crate::services::SchedulerService`] and by the API lifecycle
//! bootstrap path.
//!
//! [`otlp`] backs the OTLP exporter and [`LicenceCheckinRepository`] the
//! licence check-in. Retention deletes are not here: `database_cleanup`
//! dispatches to the purges the owning crates register.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod config_apply;
mod config_reconcile;
mod job_lock;
mod job_lock_keys;
mod jobs;
mod licence;
pub mod otlp;

pub use config_apply::{BaseRecord, ConflictRecord, ConflictRow, LastApply};
pub(crate) use config_reconcile::RECONCILE_LOCK;
pub use config_reconcile::{
    ConfigChangeRecord, ConfigChangeRow, ConfigReconcileRepository, PlaneStateRow,
    PlaneStateUpdate, PlaneTransaction,
};
pub(crate) use job_lock::JobLockGuard;
use job_lock::JobLockRepository;
pub use job_lock_keys::{LockKeyCollision, find_lock_key_collision};
pub use jobs::JobRepository;
pub use licence::{LicenceCheckinRepository, LicenceCheckinState};

use systemprompt_database::DbPool;
use systemprompt_identifiers::JobName;

use crate::error::{SchedulerError, SchedulerResult};
use crate::models::{JobRunRecord, ScheduledJob};

#[derive(Debug, Clone)]
pub struct SchedulerRepository {
    jobs: JobRepository,
    locks: JobLockRepository,
}

impl SchedulerRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            jobs: JobRepository::new(db),
            locks: JobLockRepository::new(db),
        }
    }

    pub(crate) async fn try_acquire_job_lock(
        &self,
        job_name: &JobName,
    ) -> SchedulerResult<Option<JobLockGuard>> {
        self.locks.try_acquire(job_name).await
    }

    pub async fn verify_lock_keys(&self, names: &[JobName]) -> SchedulerResult<()> {
        let keys = self.locks.lock_keys(names).await?;
        match find_lock_key_collision(&keys) {
            Some(collision) => Err(SchedulerError::LockKeyCollision {
                first: collision.first,
                second: collision.second,
                key: collision.key,
            }),
            None => Ok(()),
        }
    }

    pub(crate) async fn acquire_job_lock_within(
        &self,
        job_name: &JobName,
        wait: std::time::Duration,
    ) -> SchedulerResult<Option<JobLockGuard>> {
        self.locks.acquire_within(job_name, wait).await
    }

    pub async fn upsert_job(
        &self,
        job_name: &JobName,
        schedule: &str,
        enabled: bool,
    ) -> SchedulerResult<()> {
        self.jobs.upsert_job(job_name, schedule, enabled).await
    }

    pub async fn delete_jobs_not_in(&self, known: &[JobName]) -> SchedulerResult<u64> {
        self.jobs.delete_jobs_not_in(known).await
    }

    pub async fn find_job(&self, job_name: &JobName) -> SchedulerResult<Option<ScheduledJob>> {
        self.jobs.find_job(job_name).await
    }

    pub async fn list_enabled_jobs(&self) -> SchedulerResult<Vec<ScheduledJob>> {
        self.jobs.list_enabled_jobs().await
    }

    pub async fn update_job_execution(
        &self,
        job_name: &JobName,
        record: JobRunRecord<'_>,
    ) -> SchedulerResult<()> {
        self.jobs.update_job_execution(job_name, record).await
    }
}
