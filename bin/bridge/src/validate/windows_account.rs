//! The two Windows states an unelevated account cannot recover from on its
//! own: a machine policy that shadows this user's, and a configuration
//! directory another account created.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::Report;
use crate::config::dir_owner::{DirOwner, config_dir_owner};
use crate::config::store::{hive_report, managed_policy_store};

pub(super) fn check_policy_hives(report: &mut Report) {
    let store = managed_policy_store();
    match hive_report(store.as_ref(), crate::winproc::is_elevated()) {
        Ok(hives) if hives.is_failure() => report.fail("claude policy hive", hives.detail()),
        Ok(hives) if hives.is_warning() => report.warn("claude policy hive", hives.detail()),
        Ok(hives) => report.ok("claude policy hive", hives.detail()),
        Err(e) => report.fail("claude policy hive", &format!("registry unreadable: {e}")),
    }
}

pub(super) fn check_config_dir_owner(report: &mut Report) {
    let Some(dir) =
        crate::config::config_path().and_then(|p| p.parent().map(std::path::Path::to_path_buf))
    else {
        report.fail("config dir owner", "no config dir resolvable");
        return;
    };
    match config_dir_owner(&dir) {
        Ok(DirOwner::Absent | DirOwner::Current) => {
            report.ok(
                "config dir owner",
                &format!("{} is this account's", dir.display()),
            );
        },
        Ok(DirOwner::Foreign { owner }) => report.fail(
            "config dir owner",
            &format!(
                "{} is owned by {owner}, not this account — repair as administrator or delete \
                 the folder as an administrator and sign in again",
                dir.display()
            ),
        ),
        Err(e) => report.fail(
            "config dir owner",
            &format!("{}: owner unreadable: {e}", dir.display()),
        ),
    }
}
