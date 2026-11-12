//! Retention holds applied to the plan: each target's age cutoff is clamped
//! to the earliest watermark any registered hold reports for it, and a target
//! whose hold cannot answer is withheld for the run.
//!
//! The holds consulted are every `register_retention_hold!` registration,
//! built over the application's extension context, together with any
//! [`RetentionHolds`] the job's dependencies carry. A hold whose watermark
//! trails the age cutoff by more than the target's window again (at least a
//! day) is reported as stalled.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Duration, Utc};
use systemprompt_extension::{EffectiveCutoff, ExtensionContext, RetentionHolds, RetentionTarget};
use systemprompt_runtime::AppContext;
use systemprompt_traits::JobContext;
use tracing::{info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HoldOutcome {
    None,
    Clamped(&'static str),
    Withheld(&'static str),
}

pub(super) fn retention_holds(ctx: &JobContext, app: &AppContext) -> RetentionHolds {
    let extensions: &dyn ExtensionContext = app;
    let registered = RetentionHolds::discover(extensions);
    if let Ok(injected) = ctx.get::<RetentionHolds>() {
        return registered.merged(injected);
    }
    registered
}

pub(super) fn report(
    target: RetentionTarget,
    days: u32,
    age_cutoff: DateTime<Utc>,
    effective: &EffectiveCutoff,
) -> HoldOutcome {
    match effective {
        EffectiveCutoff::Age(_) => HoldOutcome::None,
        EffectiveCutoff::Held { cutoff, hold } => {
            info!(
                table = target.table(),
                column = target.timestamp_column(),
                retention_days = days,
                age_cutoff = %age_cutoff,
                effective_cutoff = %cutoff,
                hold,
                "Retention hold clamped the cutoff; rows at or after it are kept"
            );
            let slack = Duration::days(i64::from(days.max(1)));
            if *cutoff < age_cutoff - slack {
                warn!(
                    table = target.table(),
                    retention_days = days,
                    age_cutoff = %age_cutoff,
                    effective_cutoff = %cutoff,
                    hold,
                    "Retention hold trails the age cutoff by more than the window; the holder may be stalled"
                );
            }
            HoldOutcome::Clamped(hold)
        },
        EffectiveCutoff::Withheld { hold, error } => {
            warn!(
                table = target.table(),
                retention_days = days,
                age_cutoff = %age_cutoff,
                hold,
                error = %error,
                error_source = ?std::error::Error::source(error),
                "Retention hold could not report a watermark; the target is not pruned this run"
            );
            HoldOutcome::Withheld(hold)
        },
    }
}
