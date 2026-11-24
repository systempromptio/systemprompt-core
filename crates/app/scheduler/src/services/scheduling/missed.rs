//! The missed-run pass: each job's [`MissedRunPolicy`] applied from the last
//! instant `scheduled_job_runs` recorded for it, run on a background task
//! once the cron loop is up.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_traits::MissedRunPolicy;
use tracing::{info, warn};

use super::{SchedulerService, dispatch, registration};

impl SchedulerService {
    pub(super) async fn apply_missed_runs(&self, tickers: &[registration::Ticker]) {
        let now = self.clock.now();
        let mut due = Vec::new();
        for ticker in tickers {
            let limit = match ticker.dispatch.job.missed_runs() {
                MissedRunPolicy::Skip => continue,
                MissedRunPolicy::RunOnce => 1,
                MissedRunPolicy::CatchUp { max } => usize::try_from(max).unwrap_or(usize::MAX),
            };
            let Some(key) = ticker.dispatch.claim_policy.run_key() else {
                continue;
            };
            let job_name = &ticker.dispatch.job_name;
            let last = match self
                .repository
                .runs()
                .last_scheduled_at(job_name, &key)
                .await
            {
                Ok(Some(last)) => last,
                Ok(None) => continue,
                Err(e) => {
                    warn!(job_name = %job_name, error = %e, "cannot read the job's last recorded instant; no missed run is applied");
                    continue;
                },
            };
            let missed = ticker.instants.missed(last, now, limit);
            if !missed.is_empty() {
                info!(job_name = %job_name, count = missed.len(), "running missed instants");
                due.push((ticker.clone(), missed));
            }
        }
        if due.is_empty() {
            return;
        }
        self.app_context
            .background_tasks()
            .spawn("scheduler_missed_runs", async move {
                for (ticker, missed) in due {
                    for instant in missed {
                        ticker
                            .clone()
                            .run(dispatch::Trigger::Scheduled(instant))
                            .await;
                    }
                }
            });
    }
}
