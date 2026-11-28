//! The manifest an instance serves, and the ways it can be made wrong.
//!
//! The payload is core's `SignedManifest`, canonicalised and signed with
//! the instance's Ed25519 key over the exact payload string. It lists the
//! fixture's plugins, each with the digests of the bundle files the
//! instance serves for it; the minimal fixture has none. The harness's model
//! filter names the client families of the fixture's models, as core's
//! gateway writes it.

use std::sync::PoisonError;
use std::sync::atomic::Ordering;

use chrono::{DateTime, SecondsFormat, TimeDelta, Utc};
use serde_json::json;
use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::bridge::manifest::{
    ManifestSignature, PluginEntry, SignedManifest, SignedManifestEnvelope,
};

use crate::plugin::{FixtureError, PluginFixture};
use crate::state::InstanceState;
use crate::token;

const REDATE_AFTER_MINUTES: i64 = 4;
const VERSION_SUFFIX: &str = "5c21b7ed";
const OTHER_USER: &str = "scripted-other-user";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManifestFixture {
    fault: Option<ManifestFault>,
    plugins: Vec<PluginFixture>,
}

impl ManifestFixture {
    pub fn minimal() -> Self {
        Self::default()
    }

    pub fn with_fault(mut self, fault: ManifestFault) -> Self {
        self.fault = Some(fault);
        self
    }

    pub fn with_plugin(mut self, plugin: PluginFixture) -> Self {
        self.plugins.retain(|held| held.id != plugin.id);
        self.plugins.push(plugin);
        self
    }

    pub fn fault(&self) -> Option<ManifestFault> {
        self.fault
    }

    pub fn plugins(&self) -> &[PluginFixture] {
        &self.plugins
    }

    pub(crate) fn plugin_file(&self, plugin: &str, path: &str) -> Option<Vec<u8>> {
        self.plugins
            .iter()
            .find(|held| held.id.as_str() == plugin)?
            .served()
            .ok()?
            .remove(path)
    }
}

impl ManifestFixture {
    pub(crate) fn publication(&self, resource: &str, publication: &str) -> Option<&PluginFixture> {
        self.plugins.iter().find(|held| {
            held.published
                && held.resource_id().as_str() == resource
                && held.publication_id().as_str() == publication
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ManifestFault {
    StaleVersion,
    FutureNotBefore,
    WrongUser,
    ChangedKey,
    BadSignature,
}

#[derive(Debug)]
pub(crate) struct ServedManifest {
    pub(crate) envelope: SignedManifestEnvelope,
    pub(crate) etag: String,
}

pub(crate) fn key_is_rotated(state: &InstanceState) -> bool {
    state.manifest.fault() == Some(ManifestFault::ChangedKey)
        && state.manifests_served.load(Ordering::SeqCst) > 0
}

pub(crate) fn serve(state: &InstanceState) -> Result<ServedManifest, FixtureError> {
    let fault = state.manifest.fault();
    let served_before = state.manifests_served.load(Ordering::SeqCst) > 0;
    let dated = dated(state);
    let day = TimeDelta::days(1);

    let versioned = if fault == Some(ManifestFault::StaleVersion) && served_before {
        dated - day
    } else {
        dated
    };
    let not_before = if fault == Some(ManifestFault::FutureNotBefore) {
        dated + day
    } else {
        dated
    };
    let user_id = if fault == Some(ManifestFault::WrongUser) {
        OTHER_USER
    } else {
        state.user.user_id.as_str()
    };
    let version = format!(
        "{}-{VERSION_SUFFIX}",
        versioned.to_rfc3339_opts(SecondsFormat::Secs, true)
    );
    let protocols: Vec<&str> = {
        let mut tags: Vec<&str> = state
            .models
            .iter()
            .map(|model| model.family().as_tag())
            .collect();
        tags.sort_unstable();
        tags.dedup();
        tags
    };

    let plugins: Vec<PluginEntry> = state
        .manifest
        .plugins
        .iter()
        .map(PluginFixture::entry)
        .collect::<Result<_, _>>()?;

    let manifest: SignedManifest = serde_json::from_value(json!({
        "manifest_version": version,
        "issued_at": dated,
        "not_before": not_before,
        "user_id": user_id,
        "tenant_id": state.user.tenant_id,
        "user": {
            "id": user_id,
            "name": state.user.display_name,
            "email": state.user.email,
            "display_name": state.user.display_name,
            "roles": state.user.roles,
        },
        "plugins": plugins,
        "managed_mcp_servers": [],
        "revocations": [],
        "enabled_hosts": [HostKind::Harness],
        "host_model_protocols": { HostKind::Harness.as_str(): protocols },
    }))?;
    let payload = serde_jcs::to_string(&manifest)?;

    let key = state
        .keys
        .manifest_key(fault == Some(ManifestFault::ChangedKey));
    let signature = if fault == Some(ManifestFault::BadSignature) {
        token::sign(key, &format!("{payload} "))
    } else {
        token::sign(key, &payload)
    };
    state.manifests_served.fetch_add(1, Ordering::SeqCst);
    Ok(ServedManifest {
        etag: format!("\"{version}\""),
        envelope: SignedManifestEnvelope {
            payload,
            signature: ManifestSignature::new(signature),
        },
    })
}

fn dated(state: &InstanceState) -> DateTime<Utc> {
    let mut dated = state
        .manifest_dated
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let now = Utc::now();
    if now.signed_duration_since(*dated) > TimeDelta::minutes(REDATE_AFTER_MINUTES) {
        *dated = now;
    }
    *dated
}
