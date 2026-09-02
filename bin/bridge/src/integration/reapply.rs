//! Re-applying host profiles that are installed but no longer valid.
//!
//! Why this exists: `HostApp::install_profile` had exactly one caller in the
//! whole binary — the GUI's Re-apply button. `login` never revisited hosts, and
//! `install --apply` installs the MDM payload and the scheduled task and no
//! host profile at all, even though the stale-secret remediation names it. So a
//! profile whose loopback secret or proxy port had moved on stayed stale, every
//! request from that client 403'd, and the only cure was a button in a window.
//! This is the one path the GUI, `install --apply` and `login` now share.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use crate::config;
use crate::context::BridgeContext;
use crate::integration::host_app::{HostApp, ProbeEnv, ProfileGenInputs};
use crate::integration::profile_state::ProfileState;

pub type ModelProtocolOverrides = BTreeMap<String, Vec<String>>;

#[derive(Debug)]
pub enum Outcome {
    Reapplied,
    // Why: some hosts cannot be finished by us. macOS Claude Desktop's
    // `install_profile` shells out to `open -g <mobileconfig>`, which hands the
    // file to System Settings and returns Ok whether or not the user ever
    // approves it under Profiles. Reporting that as success is how a profile
    // stays stale while every tool insists it was refreshed, so the outcome is
    // decided by re-probing the host rather than by the call returning.
    Pending,
    Declined,
    Failed(String),
}

#[derive(Debug)]
pub struct Report {
    pub display_name: &'static str,
    pub install_action_label: &'static str,
    pub outcome: Outcome,
}

fn io_err(context: &str, e: &dyn std::fmt::Display) -> std::io::Error {
    std::io::Error::other(format!("{context}: {e}"))
}

// Why: the inputs a host profile is generated from are all live values — the
// port the proxy actually holds, the secret it will actually check, and the
// models the gateway currently offers. Rebuilding them is what makes a
// re-apply a repair rather than a rewrite of the same stale bytes.
pub async fn build_profile_inputs(
    bridge: &BridgeContext,
    host: &'static dyn HostApp,
    overrides: &ModelProtocolOverrides,
) -> std::io::Result<ProfileGenInputs> {
    let cfg = config::load();
    let loopback = bridge.proxy.loopback();
    let gateway_base_url = loopback.origin();

    // Why: a foreign install on our port must refuse — writing *our* secret
    // against *their* proxy produces exactly the 403 the profile prevents.
    let port = loopback.port();
    if let crate::proxy::peer::PeerIdentity::Foreign(who) =
        crate::proxy::peer::probe_identity(port, bridge.install_id())
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AddrInUse,
            format!(
                "127.0.0.1:{port} is served by a different {} install ({}); a profile written now \
                 would authenticate against the wrong proxy",
                crate::brand::brand().app_name,
                who.config_dir
            ),
        ));
    }

    let api_key = loopback
        .secret()
        .map(crate::ids::LoopbackSecret::into_inner)
        .map_err(|e| io_err("loopback secret", &e))?;

    let server_profile = bridge
        .gateway_client(config::gateway_url_or_default(&cfg))
        .fetch_bridge_profile()
        .await
        .map_err(|e| io_err("fetch bridge profile", &e))?;

    let surfaces = crate::gateway::model_view::effective_surfaces(
        host.id(),
        host.accepted_surfaces(),
        overrides,
    );
    let view = crate::gateway::model_view::host_model_view(&server_profile.providers, &surfaces);

    let mut headers = BTreeMap::new();
    if !surfaces.is_empty() {
        headers.insert(
            systemprompt_identifiers::headers::INFERENCE_PROTOCOL.to_owned(),
            surfaces
                .iter()
                .map(|s| s.as_tag())
                .collect::<Vec<_>>()
                .join(","),
        );
    }

    Ok(ProfileGenInputs {
        gateway_base_url,
        api_key,
        models: view.compatible_models,
        organization_uuid: server_profile.organization_uuid,
        headers,
    })
}

// Why: only hosts already carrying a profile are touched. A host the user has
// never set up is left alone — repairing what is broken is a different act from
// enrolling a new client, and login is not the place to do the second.
pub async fn reapply_stale_profiles(
    bridge: &BridgeContext,
    overrides: &ModelProtocolOverrides,
) -> Vec<Report> {
    let env = ProbeEnv::new(
        bridge.proxy.loopback(),
        std::sync::Arc::clone(&bridge.start_menu),
    );
    let mut reports = Vec::new();
    for &host in crate::integration::host_apps() {
        if !matches!(host.probe(&env).profile_state, ProfileState::Stale { .. }) {
            continue;
        }
        reports.push(Report {
            display_name: host.display_name(),
            install_action_label: host.install_action_label(),
            outcome: reapply_one(bridge, host, overrides, &env).await,
        });
    }
    reports
}

async fn reapply_one(
    bridge: &BridgeContext,
    host: &'static dyn HostApp,
    overrides: &ModelProtocolOverrides,
    env: &ProbeEnv,
) -> Outcome {
    let inputs = match build_profile_inputs(bridge, host, overrides).await {
        Ok(i) => i,
        Err(e) => return Outcome::Failed(e.to_string()),
    };
    let generated = match host.generate_profile(&inputs) {
        Ok(g) => g,
        Err(e) => return Outcome::Failed(e.to_string()),
    };
    match host.install_profile(&generated.path) {
        Ok(()) => verify(host, env),
        // Why: declining the administrator prompt is a decision, not a fault —
        // the same distinction `install::elevate` draws. Reporting it as an
        // error would make a deliberate "not now" look like a broken install.
        Err(e) if is_declined(&e) => Outcome::Declined,
        Err(e) => Outcome::Failed(e.to_string()),
    }
}

// Why: trust the probe, not the return value — see `Outcome::Pending`.
fn verify(host: &'static dyn HostApp, env: &ProbeEnv) -> Outcome {
    if matches!(host.probe(env).profile_state, ProfileState::Installed) {
        Outcome::Reapplied
    } else {
        Outcome::Pending
    }
}

fn is_declined(e: &std::io::Error) -> bool {
    e.kind() == std::io::ErrorKind::PermissionDenied
        || e.to_string().contains("cancelled the administrator")
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
    }
    out
}
