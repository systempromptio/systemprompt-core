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

// Why: without the writer every connector change is an administrator prompt;
// a machine that holds a Claude policy but no usable writer is told so here,
// where the operator looks, rather than at the next prompt.
#[cfg(target_os = "windows")]
pub fn check_policy_writer() -> Check {
    use crate::config::store::{PolicyHive, PolicyTarget, managed_policy_store};
    use crate::install::policy_writer::{WriterStatus, status};

    let machine_policy = managed_policy_store()
        .policy_key_exists(PolicyHive::Machine, PolicyTarget::Claude)
        .unwrap_or(false);
    match status() {
        WriterStatus::Ready => Check::ok(
            "policy writer",
            "registered; connector changes reach Claude Desktop without a prompt",
        ),
        WriterStatus::NotRegistered if machine_policy => Check::warn(
            "policy writer",
            "not registered; every connector change will ask for administrator approval — run \
             install --apply as Administrator with a pinned gateway key",
        ),
        WriterStatus::NotRegistered => Check::ok("policy writer", "not registered"),
        WriterStatus::Unavailable(why) => Check::fail("policy writer", why),
    }
}
