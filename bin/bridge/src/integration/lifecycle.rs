//! Switching the active gateway profile, and the user's opt-in disable.
//!
//! Hosts only ever carry the active instance. A switch removes what the
//! outgoing gateway delivered to this user — synced plugins, Cowork enable
//! keys, Claude Code plugin registrations, the managed MCP set and the
//! gateway-stamped sync state — then makes the new profile active, drops the
//! cached token and swaps the proxy's runtime config. Host routing profiles
//! stay: they point at the loopback proxy, and the caller's follow-up sync and
//! stale-profile repair rewrite their gateway-derived content.
//!
//! Disable goes further and removes the routing profiles too, so each host
//! returns to its own sign-in; the proxy then refuses to forward and sync
//! refuses to run until `enable`. It is offered only when the active
//! gateway's verified manifest allows it. Machine-scope policy (MDM, the Claude
//! Desktop managed policy) is never touched; a host still under it is reported
//! as managed rather than claimed released.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::config::gateways::{self, ActiveProfile, GatewayProfileId, ProfileError};
use crate::context::BridgeContext;

#[derive(Debug, thiserror::Error)]
pub enum LifecycleError {
    #[error(transparent)]
    Profile(#[from] ProfileError),
    #[error(transparent)]
    Install(#[from] crate::install::InstallError),
    #[error(transparent)]
    Setup(#[from] crate::auth::setup::SetupError),
    #[error(transparent)]
    ConfigRead(#[from] crate::config::ConfigReadError),
    #[error(transparent)]
    ConfigWrite(#[from] crate::config::ConfigWriteError),
    #[error("token cache: {0}")]
    Cache(#[source] std::io::Error),
    #[error("`{0}` is already the active gateway")]
    AlreadyActive(GatewayProfileId),
    #[error(
        "this gateway does not let users disable the bridge; an administrator enables it with \
         bridge_policy.allow_user_disable"
    )]
    DisableNotAllowed,
    #[error(
        "no verified manifest from the active gateway is on this device; sync once so its \
         policy on disabling is known"
    )]
    NoDeliveredPolicy,
}

/// What releasing the outgoing gateway left behind.
#[derive(Debug, Clone, Default)]
pub struct Released {
    pub foreign_plugins: Vec<String>,
    pub warnings: Vec<String>,
}

fn release_gateway(
    bridge: &BridgeContext,
    remove_profiles: bool,
) -> Result<Released, LifecycleError> {
    let foreign_plugins = crate::install::remove_synced_plugins()?;
    let mut warnings = crate::integration::uninstall::clear_gateway_content(&bridge.elevation_gate);
    if remove_profiles {
        warnings.extend(crate::integration::uninstall::remove_all_host_profiles());
    }
    crate::auth::setup::forget_gateway_state()?;
    crate::mcp_registry::clear(&bridge.mcp_registry);
    crate::auth::cache::clear().map_err(LifecycleError::Cache)?;
    Ok(Released {
        foreign_plugins,
        warnings,
    })
}

#[derive(Debug, Clone)]
pub struct SwitchReport {
    pub profile: ActiveProfile,
    pub released: Released,
}

pub fn switch(
    bridge: &BridgeContext,
    id: &GatewayProfileId,
) -> Result<SwitchReport, LifecycleError> {
    let current = gateways::active()?;
    if current.id == *id {
        return Err(LifecycleError::AlreadyActive(id.clone()));
    }
    if !gateways::list()?.iter().any(|p| p.id == *id) {
        return Err(ProfileError::Unknown(id.clone()).into());
    }
    let released = release_gateway(bridge, false)?;
    let profile = gateways::set_active(id)?;
    bridge.proxy.reload_runtime_config()?;
    tracing::info!(from = %current.id, to = %profile.id, "active gateway switched");
    Ok(SwitchReport { profile, released })
}

#[derive(Debug, Clone)]
pub struct DisableReport {
    pub released: Released,
    pub still_managed: Vec<&'static str>,
    pub already_disabled: bool,
}

pub fn disable_allowed() -> Result<(), LifecycleError> {
    let profile = gateways::active()?;
    let cfg = crate::config::read_file(&profile.config_file)?;
    let gateway = crate::config::gateway_url_or_default(&cfg);
    let delivered = gateways::read_delivered(&profile)?
        .filter(|d| {
            d.gateway
                .as_ref()
                .is_some_and(|g| crate::config::same_origin(g, &gateway))
        })
        .ok_or(LifecycleError::NoDeliveredPolicy)?;
    if delivered.allow_user_disable {
        Ok(())
    } else {
        Err(LifecycleError::DisableNotAllowed)
    }
}

pub fn disable(bridge: &BridgeContext) -> Result<DisableReport, LifecycleError> {
    let profile = gateways::active()?;
    if crate::config::read_file(&profile.config_file)?.is_disabled() {
        return Ok(DisableReport {
            released: Released::default(),
            still_managed: machine_managed(bridge),
            already_disabled: true,
        });
    }
    disable_allowed()?;
    let released = release_gateway(bridge, true)?;
    crate::config::write::edit_file(&profile.config_file, |doc| {
        crate::config::write::set(doc, &["instance", "disabled"], true)
    })?;
    bridge.proxy.reload_runtime_config()?;
    tracing::info!(profile = %profile.id, "bridge disabled by the user");
    Ok(DisableReport {
        released,
        still_managed: machine_managed(bridge),
        already_disabled: false,
    })
}

pub fn enable(bridge: &BridgeContext) -> Result<bool, LifecycleError> {
    let profile = gateways::active()?;
    if !crate::config::read_file(&profile.config_file)?.is_disabled() {
        return Ok(false);
    }
    crate::config::write::edit_file(&profile.config_file, |doc| {
        crate::config::write::remove(doc, &["instance", "disabled"])
    })?;
    bridge.proxy.reload_runtime_config()?;
    tracing::info!(profile = %profile.id, "bridge enabled by the user");
    Ok(true)
}

fn machine_managed(bridge: &BridgeContext) -> Vec<&'static str> {
    use crate::config::store::{PolicyHive, PolicyTarget};
    match bridge
        .policy_store
        .backend()
        .policy_key_exists(PolicyHive::Machine, PolicyTarget::Claude)
    {
        Ok(true) => vec!["Claude Desktop (machine policy)"],
        Ok(false) => Vec::new(),
        Err(error) => {
            tracing::warn!(%error, "machine policy unreadable; Claude Desktop may still be managed");
            vec!["Claude Desktop (machine policy unreadable)"]
        },
    }
}

/// What a gateway profile add or remove did to the machine policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineProfileRefresh {
    Current,
    Rewritten,
    AwaitingApproval,
    ManagedElsewhere { missing: Vec<GatewayProfileId> },
}

// Why: the machine policy serves every gateway profile (trust pins, model
// union), so it is re-rendered when the set of profiles changes, never on a
// switch. A policy the bridge did not write is device management's; the
// bridge never replaces it and names the profiles it does not pin.
pub async fn refresh_machine_profile(bridge: &BridgeContext) -> MachineProfileRefresh {
    let managed_by_us = machine_policy_is_ours(bridge);
    if managed_by_us == Some(false) {
        return MachineProfileRefresh::ManagedElsewhere {
            missing: unpinned_profiles(),
        };
    }
    let overrides = crate::integration::reapply::ModelProtocolOverrides::new();
    let reports = crate::integration::reapply::reapply_stale_profiles(
        bridge,
        &overrides,
        crate::integration::reapply::Attendance::Unattended,
    )
    .await;
    if reports.is_empty() {
        return MachineProfileRefresh::Current;
    }
    if reports
        .iter()
        .any(|r| matches!(r.outcome, crate::integration::reapply::Outcome::Declined))
    {
        MachineProfileRefresh::AwaitingApproval
    } else {
        MachineProfileRefresh::Rewritten
    }
}

fn machine_policy_is_ours(bridge: &BridgeContext) -> Option<bool> {
    let installed = crate::integration::claude_desktop::policy_api_key_fingerprint()?;
    let ours = bridge.proxy.loopback().secret_fingerprint()?;
    Some(installed == ours)
}

fn unpinned_profiles() -> Vec<GatewayProfileId> {
    let pins = crate::config::store::read_bridge_policy(crate::config::store::MANIFEST_TRUST_KEY)
        .ok()
        .flatten()
        .and_then(|raw| crate::config::trust::parse_policy_trust(&raw).ok())
        .unwrap_or_default();
    gateways::list()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|profile| {
            let url = profile.gateway_url.as_deref()?;
            let url = systemprompt_identifiers::ValidatedUrl::try_new(url).ok()?;
            let identity = crate::config::GatewayIdentity::new(&url).ok()?;
            (!pins.iter().any(|p| p.gateway == identity)).then_some(profile.id)
        })
        .collect()
}

#[derive(Debug, thiserror::Error)]
pub enum ReEnrolError {
    #[error(transparent)]
    LastSync(#[from] crate::last_sync::ReplayStateError),
    #[error(transparent)]
    Selection(#[from] crate::integration::enrol::SelectionError),
}

// Why: disable removed every host routing profile; enable puts back exactly
// the hosts the gateway's last delivered manifest enables and that this
// machine has, through the same enrolment `install --host` runs.
pub async fn re_enrol_delivered_hosts(
    bridge: &BridgeContext,
) -> Result<Vec<crate::integration::enrol::Report>, ReEnrolError> {
    let Some(enabled) = crate::last_sync::last_synced_enabled_hosts()? else {
        return Ok(Vec::new());
    };
    let local: Vec<_> = crate::integration::host_apps()
        .iter()
        .map(|h| h.id())
        .filter(|id| enabled.contains(id))
        .collect();
    if local.is_empty() {
        return Ok(Vec::new());
    }
    let overrides = crate::integration::reapply::ModelProtocolOverrides::new();
    Ok(crate::integration::enrol::enrol_hosts(
        bridge,
        &crate::integration::enrol::Selection::Ids(local),
        &overrides,
        Some(enabled),
    )
    .await?)
}
