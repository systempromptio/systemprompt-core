//! Doctor checks for the gateway profile: which profile is active, where its
//! PAT is stored, its release window and whether the user disabled it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::bridge::instance::WindowStatus;

use super::Check;
use crate::config::gateways::credential::PatStore;

pub(super) fn checks(cfg: &crate::config::Config) -> Vec<Check> {
    vec![
        check_profile(cfg),
        check_pat_store(cfg),
        check_release_window(),
        check_desktop_policy_owner(),
    ]
}

fn check_profile(cfg: &crate::config::Config) -> Check {
    match crate::config::gateways::active() {
        Ok(profile) if cfg.is_disabled() => Check::warn(
            "gateway profile",
            format!(
                "`{}` is disabled by the user; hosts use their own sign-in until `enable`",
                profile.id
            ),
        ),
        Ok(profile) if profile.pinned => Check::ok(
            "gateway profile",
            format!("pinned to {}", profile.config_file.display()),
        ),
        Ok(profile) => Check::ok("gateway profile", format!("`{}` active", profile.id)),
        Err(e) => Check::fail("gateway profile", e.to_string()),
    }
}

fn check_pat_store(cfg: &crate::config::Config) -> Check {
    if crate::auth::providers::pat::env_override_present() {
        return Check::ok(
            "PAT store",
            format!(
                "{} overrides the stored PAT",
                crate::brand::brand().env("PAT")
            ),
        );
    }
    match cfg.pat.and_then(|p| p.store) {
        Some(PatStore::Keyring) => Check::ok("PAT store", "OS credential store"),
        Some(PatStore::File) => Check::ok(
            "PAT store",
            "0600 file in the gateway profile (no Secret Service on this host)",
        ),
        None => Check::ok("PAT store", "no PAT stored"),
    }
}

fn check_release_window() -> Check {
    let report = match crate::instance::window::report() {
        Ok(report) => report,
        Err(e) => return Check::warn("release window", e.to_string()),
    };
    let local = crate::brand::brand().version;
    let range = format!(
        "[{}, {}]",
        report.min.as_deref().unwrap_or("any"),
        report.max.as_deref().unwrap_or("any")
    );
    if let Some(conflict) = report.conflict {
        return Check::warn(
            "release window",
            format!("gateway profiles admit no common release ({conflict}); nothing is staged"),
        );
    }
    match report.status {
        WindowStatus::Below => Check::fail(
            "release window",
            format!(
                "{local} is below {range}; sync is refused until the admitted release is installed"
            ),
        ),
        WindowStatus::Above if report.enforce_max => Check::fail(
            "release window",
            format!("{local} is above {range}; sync is refused — install the offered release"),
        ),
        WindowStatus::Above => Check::warn(
            "release window",
            format!("{local} is above {range}; install the offered release"),
        ),
        WindowStatus::Within => Check::ok("release window", format!("{local} within {range}")),
        WindowStatus::Unknown => Check::ok("release window", "no window delivered"),
    }
}

fn check_desktop_policy_owner() -> Check {
    use crate::install::mdm::clear_plan::ClearPlan;
    let ours = crate::integration::claude_desktop::installed_secret_fingerprint();
    match crate::integration::claude_desktop::policy_ownership(ours.as_deref()) {
        ClearPlan::StaleBridgePolicy => Check::warn(
            "desktop policy owner",
            "Claude Desktop runs on a previous bridge install's policy; disable and switch \
             cannot release it until it is replaced — use Replace in the app or `install --apply`",
        ),
        ClearPlan::ManagedElsewhere => Check::ok(
            "desktop policy owner",
            "set by an administrator or device management",
        ),
        ClearPlan::RemoveAll | ClearPlan::RemoveKeys(_) => {
            Check::ok("desktop policy owner", "written by this bridge")
        },
        ClearPlan::NothingWritten => Check::ok("desktop policy owner", "no managed policy"),
    }
}
