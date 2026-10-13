//! Doctor check for the `OpenCode` admin tier the bridge wrote before 2.x.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::Check;

#[must_use]
pub fn check_retired_admin_tier() -> Option<Check> {
    let managed = crate::integration::opencode::retired_admin_tier()?;
    Some(Check::warn(
        "opencode admin config",
        format!(
            "{} still carries the bridge's provider block; OpenCode 2.x ignores this file and \
             reads ~/.config/opencode/opencode.json instead. Run `uninstall --host opencode` as \
             administrator to clear it, then re-run install",
            managed.display()
        ),
    ))
}
