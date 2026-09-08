//! Enrolling a named host into the bridge from the command line.
//!
//! Why this exists: `HostApp::install_profile` was reachable only from the GUI
//! and from [`super::reapply::reapply_stale_profiles`], which by design touches
//! *only* hosts that already carry a profile. On a headless Linux box the sole
//! setup path is `install`, and it installed no host profile at all — so
//! `sync` wrote `OpenCode`'s MCP connectors while the provider block and the
//! `auth.json` key were never written, and every request the client made to
//! the loopback proxy came back `403 no loopback credential presented`. This
//! module is the enrolment half: name a host, get its profile written, whether
//! or not one was there before.
//!
//! It shares [`super::reapply::build_profile_inputs`] with the repair path, so
//! an enrolled profile and a re-applied one are generated from the same live
//! port, secret and model list.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::context::BridgeContext;
use crate::integration::host_app::{HostApp, ProbeEnv, ProfileRemoval};
use crate::integration::profile_state::ProfileState;
use crate::integration::reapply::ModelProtocolOverrides;
use crate::integration::registry::{ResolvedHost, resolve_host};
use crate::integration::sync_only::SyncOnlyAgent;

/// Which hosts the caller asked for.
#[derive(Debug, Clone)]
pub enum Selection {
    All,
    Ids(Vec<String>),
}

#[derive(Debug)]
pub enum Outcome {
    Installed,
    Pending,
    Declined,
    SyncOnly,
    NotEnabled,
    Removed,
    NothingToRemove,
    ManualStep(String),
    Failed(String),
}

#[derive(Debug)]
pub struct Report {
    pub host_id: String,
    pub display_name: &'static str,
    pub install_action_label: &'static str,
    pub outcome: Outcome,
}

impl Report {
    #[must_use]
    pub const fn is_failure(&self) -> bool {
        matches!(self.outcome, Outcome::Failed(_))
    }
}

/// What one requested id turned out to be.
#[expect(
    missing_debug_implementations,
    reason = "holds `&'static dyn HostApp`, and the trait is not Debug"
)]
#[derive(Clone, Copy)]
pub enum Target {
    Local(&'static dyn HostApp),
    SyncOnly(&'static SyncOnlyAgent),
}

impl Target {
    #[must_use]
    pub fn id(&self) -> &'static str {
        match *self {
            Self::Local(host) => host.id(),
            Self::SyncOnly(agent) => agent.id,
        }
    }
}

pub fn resolve(selection: &Selection) -> Result<Vec<Target>, String> {
    let ids = match selection {
        Selection::All => {
            return Ok(super::host_apps()
                .iter()
                .copied()
                .map(Target::Local)
                .collect());
        },
        Selection::Ids(ids) => ids,
    };
    let mut targets = Vec::with_capacity(ids.len());
    for id in ids {
        match resolve_host(id) {
            ResolvedHost::Local(host) => targets.push(Target::Local(host)),
            ResolvedHost::SyncOnly(agent) => targets.push(Target::SyncOnly(agent)),
            ResolvedHost::Suppressed => {
                return Err(format!(
                    "--host {id}: this build does not offer the '{id}' host",
                ));
            },
            ResolvedHost::Unknown => {
                return Err(format!(
                    "--host {id}: unknown host id; known ids: {}",
                    known()
                ));
            },
        }
    }
    Ok(targets)
}

fn known() -> String {
    let mut ids: Vec<&str> = super::host_apps().iter().map(|h| h.id()).collect();
    ids.extend(super::sync_only::SYNC_ONLY_AGENTS.iter().map(|a| a.id));
    ids.sort_unstable();
    ids.join(", ")
}

pub async fn enrol_hosts(
    bridge: &BridgeContext,
    selection: &Selection,
    overrides: &ModelProtocolOverrides,
    enabled: Option<Vec<String>>,
) -> Result<Vec<Report>, String> {
    let targets = resolve(selection)?;
    let env = ProbeEnv::new(
        bridge.proxy.loopback(),
        std::sync::Arc::clone(&bridge.start_menu),
    );
    let mut reports = Vec::with_capacity(targets.len());
    for target in targets {
        reports.push(match target {
            Target::SyncOnly(agent) if agent.id == CLAUDE_CODE_ID => Report {
                host_id: agent.id.to_owned(),
                display_name: agent.display_name,
                install_action_label: CLAUDE_CODE_LABEL,
                outcome: enrol_claude_code(bridge),
            },
            Target::SyncOnly(agent) => Report {
                host_id: agent.id.to_owned(),
                display_name: agent.display_name,
                install_action_label: "governed through the gateway; nothing to install locally",
                outcome: Outcome::SyncOnly,
            },
            Target::Local(host) => {
                let outcome = if enabled
                    .as_ref()
                    .is_some_and(|hosts| !hosts.iter().any(|h| h == host.id()))
                {
                    Outcome::NotEnabled
                } else {
                    enrol_one(bridge, host, overrides, &env).await
                };
                Report {
                    host_id: host.id().to_owned(),
                    display_name: host.display_name(),
                    install_action_label: host.install_action_label(),
                    outcome,
                }
            },
        });
    }
    Ok(reports)
}

const CLAUDE_CODE_ID: &str = "claude-code";
const CLAUDE_CODE_LABEL: &str = "gateway keys merged into Claude Code's settings file";

// Why: Claude Code has no HostApp — the gateway governs it — but its inference
// only reaches the gateway if its settings file names the loopback proxy.
// Before this arm the enrolment reported "governed" while `claude` kept
// talking to Anthropic directly on macOS; Linux merged the same keys from
// `install --apply` and still does.
#[cfg(unix)]
fn enrol_claude_code(bridge: &BridgeContext) -> Outcome {
    let Some(key_path) = crate::proxy::secret::secret_path() else {
        return Outcome::Failed("the loopback secret path could not be resolved".to_owned());
    };
    let gateway = bridge.proxy.loopback().origin();
    match crate::install::mdm::claude_code_settings::apply_managed_settings(&gateway, &key_path) {
        Ok(lines) => {
            for line in lines {
                tracing::info!(target: "bridge::install", detail = %line, "claude code settings");
            }
            Outcome::Installed
        },
        Err(e) => Outcome::Failed(e.to_string()),
    }
}

#[cfg(not(unix))]
fn enrol_claude_code(_bridge: &BridgeContext) -> Outcome {
    Outcome::SyncOnly
}

#[cfg(unix)]
fn remove_claude_code() -> Outcome {
    if crate::install::mdm::claude_code_settings::remove_managed_settings().is_empty() {
        Outcome::NothingToRemove
    } else {
        Outcome::Removed
    }
}

#[cfg(not(unix))]
fn remove_claude_code() -> Outcome {
    Outcome::SyncOnly
}

async fn enrol_one(
    bridge: &BridgeContext,
    host: &'static dyn HostApp,
    overrides: &ModelProtocolOverrides,
    env: &ProbeEnv,
) -> Outcome {
    let inputs = match super::reapply::build_profile_inputs(bridge, host, overrides).await {
        Ok(i) => i,
        Err(e) => return Outcome::Failed(e.to_string()),
    };
    let generated = match host.generate_profile(&inputs) {
        Ok(g) => g,
        Err(e) => return Outcome::Failed(e.to_string()),
    };
    match host.install_profile(&generated.path) {
        Ok(()) => {
            if matches!(host.probe(env).profile_state, ProfileState::Installed) {
                Outcome::Installed
            } else {
                Outcome::Pending
            }
        },
        Err(e) if super::reapply::is_declined(&e) => Outcome::Declined,
        Err(e) => Outcome::Failed(e.to_string()),
    }
}

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
    }
    out
}

pub fn remove_host_profiles(selection: &Selection) -> Result<Vec<Report>, String> {
    let targets = resolve(selection)?;
    Ok(targets
        .into_iter()
        .map(|target| match target {
            Target::SyncOnly(agent) if agent.id == CLAUDE_CODE_ID => Report {
                host_id: agent.id.to_owned(),
                display_name: agent.display_name,
                install_action_label: CLAUDE_CODE_LABEL,
                outcome: remove_claude_code(),
            },
            Target::SyncOnly(agent) => Report {
                host_id: agent.id.to_owned(),
                display_name: agent.display_name,
                install_action_label: "governed through the gateway; nothing local to remove",
                outcome: Outcome::SyncOnly,
            },
            Target::Local(host) => Report {
                host_id: host.id().to_owned(),
                display_name: host.display_name(),
                install_action_label: host.install_action_label(),
                outcome: match host.remove_profile() {
                    Ok(ProfileRemoval::Removed { .. }) => Outcome::Removed,
                    Ok(ProfileRemoval::NothingToRemove) => Outcome::NothingToRemove,
                    Ok(ProfileRemoval::ManualStepRequired { instruction }) => {
                        Outcome::ManualStep(instruction)
                    },
                    Err(e) if super::reapply::is_declined(&e) => Outcome::Declined,
                    Err(e) => Outcome::Failed(e.to_string()),
                },
            },
        })
        .collect())
}
