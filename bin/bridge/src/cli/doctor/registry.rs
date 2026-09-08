//! Doctor check for the Windows managed-policy hives: which of `HKLM` and
//! `HKCU` hold `SOFTWARE\Policies\Claude`, and whether the machine key is
//! shadowing a per-user copy that an unelevated bridge wrote.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::cli::doctor::Check;

#[cfg(target_os = "windows")]
pub fn check_policy_hives() -> Option<Check> {
    use crate::config::store::{PolicyHive, managed_policy_store};

    const PROBE_KEYS: &[&str] = &["inferenceGatewayBaseUrl", "managedMcpServers"];
    let store = managed_policy_store();
    let read = |hive: PolicyHive| match store.read_policy_document(hive, PROBE_KEYS) {
        Ok(doc) => Ok(doc),
        Err(e) => Err(format!("{}: {e}", hive.label())),
    };
    let (machine, user) = match (read(PolicyHive::Machine), read(PolicyHive::User)) {
        (Ok(m), Ok(u)) => (m, u),
        (Err(e), _) | (_, Err(e)) => {
            return Some(Check::fail(
                "claude policy hive",
                format!("registry unreadable: {e}"),
            ));
        },
    };
    let elevated = crate::winproc::is_elevated();
    Some(match (machine.is_empty(), user.is_empty()) {
        (true, true) => Check::warn(
            "claude policy hive",
            "no Claude policy in HKLM or HKCU — sync has not written one yet",
        ),
        (false, true) => Check::ok(
            "claude policy hive",
            "HKLM holds the Claude policy (machine-wide)",
        ),
        (true, false) if elevated => Check::warn(
            "claude policy hive",
            "HKCU holds the Claude policy but this process is elevated and will write HKLM, \
             which then shadows the per-user copy",
        ),
        (true, false) => Check::ok(
            "claude policy hive",
            "HKCU holds the Claude policy (per-user; honoured while no HKLM policy exists)",
        ),
        (false, false) if machine == user => Check::ok(
            "claude policy hive",
            "HKLM and HKCU both hold the Claude policy with matching values",
        ),
        (false, false) => Check::fail(
            "claude policy hive",
            "HKLM shadows a different HKCU policy — Claude reads HKLM only; remove the stale \
             key or re-run `install --apply` as Administrator",
        ),
    })
}

#[cfg(not(target_os = "windows"))]
pub const fn check_policy_hives() -> Option<Check> {
    None
}
