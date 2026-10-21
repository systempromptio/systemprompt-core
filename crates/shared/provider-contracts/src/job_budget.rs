//! [`DbBudget`]: the database limits a job's run transactions carry.
//!
//! A job declares its budget through [`crate::Job::db_budget`]; the
//! scheduler hands the job a budgeted database handle that applies the
//! limits with `SET LOCAL` to every transaction the job begins through it,
//! so a runaway statement is cancelled and rolled back instead of competing
//! with live traffic on the primary.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DbBudget {
    pub statement_timeout: Duration,
    pub lock_timeout: Option<Duration>,
    pub work_mem_kb: Option<u32>,
}

impl DbBudget {
    #[must_use]
    pub const fn new(statement_timeout: Duration) -> Self {
        Self {
            statement_timeout,
            lock_timeout: None,
            work_mem_kb: None,
        }
    }

    #[must_use]
    pub const fn with_lock_timeout(mut self, lock_timeout: Duration) -> Self {
        self.lock_timeout = Some(lock_timeout);
        self
    }

    #[must_use]
    pub const fn with_work_mem_kb(mut self, work_mem_kb: u32) -> Self {
        self.work_mem_kb = Some(work_mem_kb);
        self
    }
}
