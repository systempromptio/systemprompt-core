//! Enrolling a named host into the bridge from the command line.
//!
//! Name a host and get its profile written, whether or not one was there
//! before. Shares [`super::reapply::build_profile_inputs`] with the repair
//! path, so an enrolled profile and a re-applied one are generated from the
//! same live port, secret and model list.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod claude_code;
mod render;

pub use render::render;

use systemprompt_models::bridge::host::{HostKind, UnknownHostKind};

use crate::context::BridgeContext;
use crate::integration::host_app::{HostApp, ProbeEnv, ProfileRemoval};
use crate::integration::profile_state::ProfileState;
use crate::integration::reapply::{ModelProtocolOverrides, ProfileFailure};
use crate::integration::registry::{ResolvedHost, resolve_host};
use crate::integration::sync_only::SyncOnlyAgent;

/// Which hosts the caller asked for.
#[derive(Debug, Clone)]
pub enum Selection {
    All,
    Ids(Vec<HostKind>),
}

impl Selection {
    pub fn parse_ids(raw: &[String]) -> Result<Self, SelectionError> {
        raw.iter()
            .map(|id| {
                id.parse::<HostKind>()
                    .map_err(|UnknownHostKind(raw)| SelectionError::Unknown {
                        id: raw,
                        known: known(),
                    })
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Self::Ids)
    }
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
    Failed(ProfileFailure),
}

#[derive(Debug)]
pub struct Report {
    pub host_id: HostKind,
    pub display_name: &'static str,
    pub install_action_label: &'static str,
    pub outcome: Outcome,
    pub warnings: Vec<String>,
}

impl Report {
    #[must_use]
    pub const fn is_failure(&self) -> bool {
        matches!(self.outcome, Outcome::Failed(_))
    }
}

/// A `--host` selection that names a host this build cannot act on.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SelectionError {
    #[error("--host {id}: this build does not offer the '{id}' host")]
    Suppressed { id: String },
    #[error("--host {id}: unknown host id; known ids: {known}")]
    Unknown { id: String, known: String },
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
    pub fn id(&self) -> HostKind {
        match *self {
            Self::Local(host) => host.id(),
            Self::SyncOnly(agent) => agent.id,
        }
    }
}

pub fn resolve(selection: &Selection) -> Result<Vec<Target>, SelectionError> {
    let ids = match selection {
        Selection::All => {
            return Ok(super::host_apps()
                .iter()
                .copied()
                .map(Target::Local)
                .chain(claude_code::installed_agent().map(Target::SyncOnly))
                .collect());
        },
        Selection::Ids(ids) => ids,
    };
    let mut targets = Vec::with_capacity(ids.len());
    for &id in ids {
        match resolve_host(id) {
            ResolvedHost::Local(host) => targets.push(Target::Local(host)),
            ResolvedHost::SyncOnly(agent) => targets.push(Target::SyncOnly(agent)),
            ResolvedHost::Suppressed => {
                return Err(SelectionError::Suppressed {
                    id: id.as_str().to_owned(),
                });
            },
            ResolvedHost::Unknown => {
                return Err(SelectionError::Unknown {
                    id: id.as_str().to_owned(),
                    known: known(),
                });
            },
        }
    }
    Ok(targets)
}

fn known() -> String {
    let mut ids: Vec<&str> = super::host_apps().iter().map(|h| h.id().as_str()).collect();
    ids.extend(
        super::sync_only::SYNC_ONLY_AGENTS
            .iter()
            .map(|a| a.id.as_str()),
    );
    ids.sort_unstable();
    ids.join(", ")
}

pub async fn enrol_hosts(
    bridge: &BridgeContext,
    selection: &Selection,
    overrides: &ModelProtocolOverrides,
    enabled: Option<Vec<String>>,
) -> Result<Vec<Report>, SelectionError> {
    let targets = resolve(selection)?;
    let env = ProbeEnv::for_bridge(bridge);
    let not_enabled = |id: HostKind| {
        enabled
            .as_ref()
            .is_some_and(|hosts| !hosts.iter().any(|h| h == id.as_str()))
    };
    // Why: `--host claude-code` has always enrolled regardless of the
    // instance's enabled hosts; only the implicit `all` selection respects it.
    let claude_code_gated =
        matches!(selection, Selection::All) && not_enabled(HostKind::ClaudeCode);
    let mut reports = Vec::with_capacity(targets.len());
    for target in targets {
        reports.push(match target {
            Target::SyncOnly(agent) if agent.id == HostKind::ClaudeCode => {
                if claude_code_gated {
                    claude_code::not_enabled_report()
                } else {
                    claude_code::enrol_report(bridge)
                }
            },
            Target::SyncOnly(agent) => Report {
                host_id: agent.id,
                display_name: agent.display_name,
                install_action_label: "governed through the gateway; nothing to install locally",
                outcome: Outcome::SyncOnly,
                warnings: Vec::new(),
            },
            Target::Local(host) => {
                let (outcome, warnings) = if not_enabled(host.id()) {
                    (Outcome::NotEnabled, Vec::new())
                } else {
                    enrol_one(bridge, host, overrides, &env).await
                };
                Report {
                    host_id: host.id(),
                    display_name: host.display_name(),
                    install_action_label: host.install_action_label(),
                    outcome,
                    warnings,
                }
            },
        });
    }
    Ok(reports)
}

async fn enrol_one(
    bridge: &BridgeContext,
    host: &'static dyn HostApp,
    overrides: &ModelProtocolOverrides,
    env: &ProbeEnv,
) -> (Outcome, Vec<String>) {
    let inputs = match super::reapply::build_profile_inputs(bridge, host, overrides).await {
        Ok(i) => i,
        Err(e) => return (Outcome::Failed(e.into()), Vec::new()),
    };
    let generated = match host.generate_profile(&inputs) {
        Ok(g) => g,
        Err(e) => return (Outcome::Failed(e.into()), Vec::new()),
    };
    match host.install_profile(&generated.path) {
        Ok(installed) => {
            let outcome = if matches!(host.probe(env).profile_state, ProfileState::Installed) {
                Outcome::Installed
            } else {
                Outcome::Pending
            };
            (outcome, installed.warnings)
        },
        Err(e) if e.is_refusal() => (Outcome::Declined, Vec::new()),
        Err(e) => (Outcome::Failed(e.into()), Vec::new()),
    }
}

pub fn remove_host_profiles(selection: &Selection) -> Result<Vec<Report>, SelectionError> {
    let targets = resolve(selection)?;
    Ok(targets
        .into_iter()
        .map(|target| match target {
            Target::SyncOnly(agent) if agent.id == HostKind::ClaudeCode => {
                claude_code::removal_report()
            },
            Target::SyncOnly(agent) => Report {
                host_id: agent.id,
                display_name: agent.display_name,
                install_action_label: "governed through the gateway; nothing local to remove",
                outcome: Outcome::SyncOnly,
                warnings: Vec::new(),
            },
            Target::Local(host) => Report {
                host_id: host.id(),
                display_name: host.display_name(),
                install_action_label: host.install_action_label(),
                outcome: match host.remove_profile() {
                    Ok(ProfileRemoval::Removed { .. }) => Outcome::Removed,
                    Ok(ProfileRemoval::NothingToRemove) => Outcome::NothingToRemove,
                    Ok(ProfileRemoval::ManualStepRequired { instruction }) => {
                        Outcome::ManualStep(instruction)
                    },
                    Err(e) if e.is_refusal() => Outcome::Declined,
                    Err(e) => Outcome::Failed(e.into()),
                },
                warnings: Vec::new(),
            },
        })
        .collect())
}
