//! This replica's configuration fingerprint on `/readyz` and `/metrics`.
//!
//! `/readyz` carries the fingerprint as its `config` object.
//! `systemprompt_config_info{profile_sha,secret_keys_sha,services_sha,
//! secrets_generation}` is 1 for the configuration currently loaded and 0 for
//! one this process ran earlier, so `count by (profile_sha)
//! (systemprompt_config_info == 1)` shows whether a rollout or a secret
//! rotation has converged across replicas.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_config::ConfigFingerprint;
use systemprompt_loader::{BootContext, ServicesProvenance};

pub(super) const CONFIG_INFO: &str = "systemprompt_config_info";

pub(crate) fn current(boot: &BootContext) -> Option<ConfigFingerprint> {
    ConfigFingerprint::current(boot.profile(), boot.secrets(), services_sha(boot))
}

fn services_sha(boot: &BootContext) -> Option<String> {
    match boot.services_root().map(|root| &root.provenance) {
        Some(
            ServicesProvenance::Fetched { composed_hash, .. }
            | ServicesProvenance::LastGood { composed_hash, .. },
        ) => Some(composed_hash.clone()),
        _ => None,
    }
}

pub(super) fn describe() {
    metrics::describe_gauge!(
        CONFIG_INFO,
        "1 for the configuration this replica runs (profile, secret names, services tree)"
    );
}

pub(super) fn publish(boot: &BootContext, last: &mut Option<ConfigFingerprint>) {
    let Some(fingerprint) = current(boot) else {
        return;
    };
    if last.as_ref() == Some(&fingerprint) {
        return;
    }
    if let Some(previous) = last.take() {
        set(&previous, 0.0);
    }
    set(&fingerprint, 1.0);
    *last = Some(fingerprint);
}

fn set(fingerprint: &ConfigFingerprint, value: f64) {
    metrics::gauge!(
        CONFIG_INFO,
        "profile_sha" => fingerprint.profile_sha.clone(),
        "secret_keys_sha" => fingerprint.secret_keys_sha.clone(),
        "services_sha" => fingerprint.services_sha.clone().unwrap_or_default(),
        "secrets_generation" => fingerprint.secrets_generation.to_string(),
    )
    .set(value);
}
