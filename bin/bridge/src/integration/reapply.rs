//! Re-applying host profiles that are installed but no longer valid.
//!
//! The one path the GUI's Re-apply button, `install --apply` and `login`
//! share, so a profile whose loopback secret or proxy port moved on is
//! repaired from any of them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use crate::config;
use crate::context::BridgeContext;
pub use crate::integration::host_app::ModelProtocolOverrides;
use crate::integration::host_app::{
    HostApp, HostAppError, ProbeEnv, ProfileFacts, ProfileFactsError, ProfileGenInputs,
};
use crate::integration::profile_state::ProfileState;


#[derive(Debug, Clone)]
pub enum Outcome {
    Reapplied,
    Pending,
    Declined,
    Failed(Arc<ProfileFailure>),
}

/// Why writing a host's profile failed — as opposed to being declined.
#[derive(Debug, thiserror::Error)]
pub enum ProfileFailure {
    #[error(transparent)]
    Inputs(#[from] ProfileInputsError),
    #[error(transparent)]
    Host(#[from] HostAppError),
    #[error(transparent)]
    Settings(#[from] crate::install::mdm::MdmError),
}

#[derive(Debug, Clone)]
pub struct Report {
    pub display_name: &'static str,
    pub install_action_label: &'static str,
    pub outcome: Outcome,
    pub warnings: Vec<String>,
}

/// Why the inputs a host profile is rendered from could not be assembled.
#[derive(Debug, thiserror::Error)]
pub enum ProfileInputsError {
    #[error("load config: {0}")]
    Config(#[from] config::ConfigReadError),
    #[error(
        "127.0.0.1:{port} is served by a different {app} install ({config_dir}); a profile \
         written now would authenticate against the wrong proxy"
    )]
    ForeignProxy {
        port: u16,
        app: &'static str,
        config_dir: String,
    },
    #[error("fetch bridge profile: {0}")]
    BridgeProfile(#[source] crate::auth::profile::ProfileFetchError),
    #[error(transparent)]
    Facts(#[from] ProfileFactsError),
}

pub async fn build_profile_inputs(
    bridge: &BridgeContext,
    host: &'static dyn HostApp,
    overrides: &ModelProtocolOverrides,
) -> Result<ProfileGenInputs, ProfileInputsError> {
    let cfg = config::load()?;
    let loopback = bridge.proxy.loopback();

    let port = loopback.port();
    if let crate::proxy::peer::PeerIdentity::Foreign(who) =
        crate::proxy::peer::probe_identity(port, bridge.install_id())
    {
        return Err(ProfileInputsError::ForeignProxy {
            port,
            app: crate::brand::brand().app_name,
            config_dir: who.config_dir,
        });
    }

    let gateway_url = config::gateway_url_or_default(&cfg);
    let server_profile = crate::auth::profile::fetch_signed_in(&bridge.gateway_client(gateway_url))
        .await
        .map_err(ProfileInputsError::BridgeProfile)?;

    let mut facts = ProfileFacts::load(
        loopback,
        &bridge.mcp_registry.load(),
        overrides.clone(),
        Some(&bridge.warn_once),
    )?;
    facts.profile = Some(server_profile);
    Ok(facts.inputs_for(host.profile_identity()))
}

/// Whether the user asked for this repair.
///
/// An attended repair may raise the operating system's own prompts (an
/// administrator approval, a profile to accept in System Settings); an
/// unattended one never does, and reports `Declined` for a host that would
/// need one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attendance {
    Attended,
    Unattended,
}

pub async fn reapply_stale_profiles(
    bridge: &BridgeContext,
    overrides: &ModelProtocolOverrides,
    attendance: Attendance,
) -> Vec<Report> {
    let env = ProbeEnv::for_bridge(bridge);
    let mut reports = Vec::new();
    for &host in crate::integration::host_apps() {
        if !matches!(host.probe(&env).profile_state, ProfileState::Stale { .. }) {
            continue;
        }
        reports.push(reapply_host(bridge, host, overrides, &env, attendance).await);
    }
    reports
}

pub async fn reapply_host(
    bridge: &BridgeContext,
    host: &'static dyn HostApp,
    overrides: &ModelProtocolOverrides,
    env: &ProbeEnv,
    attendance: Attendance,
) -> Report {
    let (outcome, warnings) = reapply_one(bridge, host, overrides, env, attendance).await;
    Report {
        display_name: host.display_name(),
        install_action_label: host.install_action_label(),
        outcome,
        warnings,
    }
}

async fn reapply_one(
    bridge: &BridgeContext,
    host: &'static dyn HostApp,
    overrides: &ModelProtocolOverrides,
    env: &ProbeEnv,
    attendance: Attendance,
) -> (Outcome, Vec<String>) {
    let inputs = match build_profile_inputs(bridge, host, overrides).await {
        Ok(i) => i,
        Err(e) => return (failed(e), Vec::new()),
    };
    let generated = match host.generate_profile(&inputs) {
        Ok(g) => g,
        Err(e) => return (failed(e), Vec::new()),
    };
    let installed = match attendance {
        Attendance::Attended => host.install_profile(&generated.path),
        Attendance::Unattended => host.install_profile_unattended(&generated.path),
    };
    match installed {
        Ok(installed) => (verify(host, env), installed.warnings),
        Err(e) if e.is_refusal() => (Outcome::Declined, Vec::new()),
        Err(e) => (failed(e), Vec::new()),
    }
}

fn failed(e: impl Into<ProfileFailure>) -> Outcome {
    Outcome::Failed(Arc::new(e.into()))
}

fn verify(host: &'static dyn HostApp, env: &ProbeEnv) -> Outcome {
    if matches!(host.probe(env).profile_state, ProfileState::Installed) {
        Outcome::Reapplied
    } else {
        Outcome::Pending
    }
}

#[must_use]
pub fn render(reports: &[Report]) -> String {
    if reports.is_empty() {
        return "host profiles: all installed profiles are current".to_owned();
    }
    let mut out = String::from("host profiles re-applied:\n");
    for r in reports {
        let line = match &r.outcome {
            Outcome::Reapplied => format!("  [ok      ] {} — profile refreshed", r.display_name),
            Outcome::Pending => format!(
                "  [pending ] {} — handed to the OS; approve it to finish ({})",
                r.display_name, r.install_action_label
            ),
            Outcome::Declined => format!(
                "  [declined] {} — administrator approval refused; re-run to retry",
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
