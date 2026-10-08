//! The operator-facing text for a host enrolment or removal run: one line per
//! host, its outcome and any warnings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{Outcome, Report};

#[must_use]
pub fn render(reports: &[Report]) -> String {
    if reports.is_empty() {
        return "host enrolment: no hosts selected".to_owned();
    }
    let mut out = String::from("host enrolment:\n");
    for r in reports {
        let line = match &r.outcome {
            Outcome::Installed => format!(
                "  [ok      ] {} — profile installed ({})",
                r.display_name, r.install_action_label
            ),
            Outcome::Pending => format!(
                "  [pending ] {} — handed to the OS; approve it to finish ({})",
                r.display_name, r.install_action_label
            ),
            Outcome::Declined => format!(
                "  [declined] {} — administrator approval refused; re-run to retry",
                r.display_name
            ),
            Outcome::SyncOnly => format!(
                "  [ok      ] {} — governed through the gateway; skills and plugins arrive via \
                 sync",
                r.display_name
            ),
            Outcome::NotEnabled => format!(
                "  [skipped ] {} — the instance does not enable this host for you; ask an \
                 administrator to enable '{}'",
                r.display_name, r.host_id
            ),
            Outcome::Removed => format!(
                "  [ok      ] {} — bridge-owned settings removed",
                r.display_name
            ),
            Outcome::NothingToRemove => format!(
                "  [ok      ] {} — nothing of ours left to remove",
                r.display_name
            ),
            Outcome::ManualStep(instruction) => format!(
                "  [pending ] {} — finish by hand: {instruction}",
                r.display_name
            ),
            Outcome::Failed(e) => format!("  [failed  ] {} — {e}", r.display_name),
        };
        out.push_str(&line);
        out.push('\n');
        for warning in &r.warnings {
            out.push_str(&format!("  [warning ] {} — {warning}\n", r.display_name));
        }
    }
    out
}
