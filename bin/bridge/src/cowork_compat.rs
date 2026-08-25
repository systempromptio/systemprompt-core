//! Every constant and version-pinned behaviour the bridge mirrors from the
//! Cowork desktop app, in one place.
//!
//! Cowork ships no compatibility contract; these values were observed by
//! inspecting `app.asar` and live installs, and each can silently change under
//! a Cowork update. When one drifts, `bridge doctor` is the detector and this
//! module is the single place to correct it.
//!
//! Behaviours pinned here besides the constants below:
//!
//! - **Empty `<dict/>` oauth** — Cowork reads an empty `<dict/>` under a
//!   managed MCP server's `oauth` key as "needs OAuth, do well-known
//!   discovery"; omitting the key disables discovery entirely
//!   (`install/mdm/macos.rs`).
//! - **`installationPreference`** — a synced `plugin.json` lacking the field
//!   triggers Cowork's "Contact an organization owner" tooltip under MDM
//!   (`cli/doctor/cowork.rs`).
//! - **Workspace trust** — without a pre-trusted workspace
//!   (`allowedWorkspaceFolders` with `isDefaultSelected`) Cowork falls back to
//!   protected host paths and blocks on `request_cowork_directory`
//!   (`install/mdm/mod.rs`).
//! - **OAuth over HTTPS only** — Cowork's OAuth flow rejects a non-HTTPS
//!   authorize URL, so managed servers must point at the loopback proxy that
//!   injects the gateway JWT (`install/mdm/mod.rs`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

// Why: Cowork's fixed sentinel for the personal org-session dir; if it ever
// changes, `pick_target` falls back to mtime and `bridge doctor` flags the
// mismatch — update the literal here to whatever Cowork now hard-codes.
pub const PERSONAL_SESSION_UUID: &str = "00000000-0000-4000-8000-000000000001";

// Why: Cowork >= 1.22209 ignores HKCU entirely once HKLM\SOFTWARE\Policies\
// Claude exists, so policy writes must target HKLM and clear any stale HKCU
// copy.
pub const POLICY_SUBKEY: &str = r"SOFTWARE\Policies\Claude";
pub const HKCU_POLICY_KEY: &str = r"HKCU\SOFTWARE\Policies\Claude";
pub const HKLM_POLICY_KEY: &str = r"HKLM\SOFTWARE\Policies\Claude";
