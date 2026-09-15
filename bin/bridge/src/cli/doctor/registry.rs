//! Windows policy scope and shadowing diagnostics.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::cli::doctor::Check;

#[cfg(target_os = "windows")]
pub fn check_policy_hives() -> Check {
    use crate::config::store::{hive_report, managed_policy_store};

    let store = managed_policy_store();
    let report = match hive_report(store.as_ref(), crate::winproc::is_elevated()) {
        Ok(report) => report,
        Err(e) => {
            return Check::fail("claude policy hive", format!("registry unreadable: {e}"));
        },
    };
    if report.is_failure() {
        Check::fail("claude policy hive", report.detail())
    } else if report.is_warning() {
        Check::warn("claude policy hive", report.detail())
    } else {
        Check::ok("claude policy hive", report.detail())
    }
}
