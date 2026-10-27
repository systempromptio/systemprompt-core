//! Whether the installed `OpenCode` keeps the config it was given.
//!
//! `OpenCode` drops a malformed policy or an unknown key without a message,
//! so a typo in a `deny` leaves the action open. The written documents (the
//! user `opencode.json`, or a JSON managed file) are compared against what
//! `opencode debug config` reports as loaded: the gateway provider must be
//! present, and every written policy must survive with its order intact
//! (`OpenCode` evaluates policies last-match-wins, so order is meaning).
//!
//! `opencode debug config` may be answered by a background `opencode serve`
//! started earlier with another environment. That service is not the
//! bridge's to stop, so a mismatch names it as a possible cause instead.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

use super::config::{self, PROVIDER_ID};
use super::tier::{ProviderTier, provider_tier};
use super::version::{ClientProbeError, ClientRun, run_client};

const DEADLINE: Duration = Duration::from_secs(20);

/// A policy as `(effect, action, resource)`, compared verbatim so an action
/// the bridge does not model still round-trips.
pub type PolicyKey = (String, String, String);

/// One written document the client must have loaded.
#[derive(Debug, Clone)]
pub struct WrittenConfig {
    pub source: PathBuf,
    pub policies: Vec<PolicyKey>,
    pub provider: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Loss {
    Provider {
        source: PathBuf,
    },
    Policy {
        source: PathBuf,
        index: usize,
        policy: String,
    },
}

impl std::fmt::Display for Loss {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Provider { source } => write!(
                f,
                "provider `{PROVIDER_ID}` from {} is not loaded",
                source.display()
            ),
            Self::Policy {
                source,
                index,
                policy,
            } => write!(
                f,
                "policies[{index}] in {} is not loaded: {policy}",
                source.display()
            ),
        }
    }
}

#[derive(Debug)]
pub enum RoundTrip {
    NotInstalled,
    NothingWritten,
    Unreadable(ClientProbeError),
    Kept,
    Lost(Vec<Loss>),
}

#[must_use]
pub fn probe_round_trip() -> RoundTrip {
    let written = written_configs();
    if written.is_empty() {
        return RoundTrip::NothingWritten;
    }
    match run_client(&["debug", "config"], DEADLINE) {
        ClientRun::NotInstalled => RoundTrip::NotInstalled,
        ClientRun::Failed(error) => RoundTrip::Unreadable(error),
        ClientRun::Output(text) => match serde_json::from_str::<Value>(json_body(&text)) {
            Ok(loaded) => compare(&written, &loaded),
            Err(error) => RoundTrip::Unreadable(ClientProbeError::Config(error)),
        },
    }
}

#[must_use]
pub fn compare(written: &[WrittenConfig], loaded: &Value) -> RoundTrip {
    let loaded_policies = loaded_policies(loaded);
    let provider_loaded = loaded
        .get("providers")
        .or_else(|| loaded.get("provider"))
        .and_then(|providers| providers.get(PROVIDER_ID))
        .is_some();
    let mut lost = Vec::new();
    for doc in written {
        if doc.provider && !provider_loaded {
            lost.push(Loss::Provider {
                source: doc.source.clone(),
            });
        }
        let mut cursor = 0;
        for (index, policy) in doc.policies.iter().enumerate() {
            match loaded_policies[cursor..].iter().position(|p| p == policy) {
                Some(offset) => cursor += offset + 1,
                None => lost.push(Loss::Policy {
                    source: doc.source.clone(),
                    index,
                    policy: describe(policy),
                }),
            }
        }
    }
    if lost.is_empty() {
        RoundTrip::Kept
    } else {
        RoundTrip::Lost(lost)
    }
}

#[must_use]
pub fn written_config(source: &Path, document: &Value) -> WrittenConfig {
    WrittenConfig {
        source: source.to_path_buf(),
        policies: document
            .get("policies")
            .and_then(Value::as_array)
            .map(|items| items.iter().map(policy_key).collect())
            .unwrap_or_default(),
        provider: document
            .get("providers")
            .and_then(|providers| providers.get(PROVIDER_ID))
            .is_some(),
    }
}

fn written_configs() -> Vec<WrittenConfig> {
    let path = match provider_tier() {
        ProviderTier::Managed(source) => source,
        ProviderTier::User => config::user_config_path(),
    };
    if path.extension().and_then(|e| e.to_str()) != Some("json") {
        return Vec::new();
    }
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    serde_json::from_str::<Value>(text)
        .map(|document| vec![written_config(&path, &document)])
        .unwrap_or_default()
}

// JSON: a policy entry is host-owned JSON; a missing field reads as empty, so
// a malformed written entry never matches a loaded one and reports as lost.
fn policy_key(item: &Value) -> PolicyKey {
    let field = |name: &str| {
        item.get(name)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    (field("effect"), field("action"), field("resource"))
}

fn loaded_policies(loaded: &Value) -> Vec<PolicyKey> {
    loaded
        .get("policies")
        .and_then(Value::as_array)
        .map(|items| items.iter().map(policy_key).collect())
        .unwrap_or_default()
}

fn describe((effect, action, resource): &PolicyKey) -> String {
    format!("{effect} {action} {resource}")
}

fn json_body(text: &str) -> &str {
    text.find('{').map_or(text, |start| &text[start..])
}
