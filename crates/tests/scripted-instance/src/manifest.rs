//! The manifest an instance serves, and the ways it can be made wrong.
//!
//! The payload is core's `SignedManifest`, canonicalised and signed with
//! the instance's Ed25519 key over the exact payload string.

use std::sync::PoisonError;
use std::sync::atomic::Ordering;

use chrono::{DateTime, SecondsFormat, TimeDelta, Utc};
use serde_json::json;
use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::bridge::manifest::{
    ManifestSignature, SignedManifest, SignedManifestEnvelope,
};

use crate::state::InstanceState;
use crate::token;

const REDATE_AFTER_MINUTES: i64 = 4;
const VERSION_SUFFIX: &str = "5c21b7ed";
const OTHER_USER: &str = "scripted-other-user";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManifestFixture {
    fault: Option<ManifestFault>,
}

impl ManifestFixture {
    pub fn minimal() -> Self {
        Self::default()
    }

    pub fn with_fault(mut self, fault: ManifestFault) -> Self {
        self.fault = Some(fault);
        self
    }

    pub fn fault(&self) -> Option<ManifestFault> {
        self.fault
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

pub(crate) fn serve(state: &InstanceState) -> Result<ServedManifest, serde_json::Error> {
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
            .map(|model| model.protocol.as_tag())
            .collect();
        tags.sort_unstable();
        tags.dedup();
        tags
    };

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
        "plugins": [],
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
