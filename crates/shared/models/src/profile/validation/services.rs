//! Checks for the `services.sources:` and `secrets:` profile blocks.
//!
//! A non-bundled services source must carry exactly one form of provenance —
//! a pinned archive digest or a set of signing keys — because accepting both
//! would let a rotated signing key silently override a pin, and accepting
//! neither would fetch executable configuration over the network unverified.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::str::FromStr;

use base64::Engine;

use super::super::{
    BundleVerification, MAX_VAULT_RETRIES, MAX_VAULT_TIMEOUT_SECS, OciReference, Profile, VaultAuth,
};
use crate::net::{trusted_http_hosts_from_env, validate_outbound_url_with_trust};

const ED25519_PUBLIC_KEY_LEN: usize = 32;

impl Profile {
    pub(crate) fn validate_services_sources(&self, errors: &mut Vec<String>, is_cloud: bool) {
        let trusted = trusted_http_hosts_from_env();
        let mut seen: BTreeSet<&str> = BTreeSet::new();

        for source in &self.services.sources {
            if source.name.trim().is_empty() {
                errors.push("services.sources[].name is required".to_owned());
            } else if !seen.insert(source.name.as_str()) {
                errors.push(format!(
                    "services.sources has duplicate name '{}'; names key the bundle cache and \
                     must be unique",
                    source.name
                ));
            }

            let label = format!("services.sources[{}]", source.name);
            if !source.is_exactly_one() {
                errors.push(format!("{label} requires exactly one of https or oci"));
            }
            if let Some(https) = source.https.as_ref()
                && let Err(e) = validate_outbound_url_with_trust(&https.url, &trusted)
            {
                errors.push(format!("{label}.https.url is not fetchable: {e}"));
            }
            if let Some(oci) = source.oci.as_ref()
                && let Err(e) = OciReference::from_str(&oci.reference)
            {
                errors.push(format!("{label}.oci.reference is invalid: {e}"));
            }

            if let Some(verify) = source.verification() {
                validate_verification(&label, verify, errors);
            }
        }

        self.validate_services_cache_dir(errors, is_cloud);
    }

    fn validate_services_cache_dir(&self, errors: &mut Vec<String>, is_cloud: bool) {
        let Some(dir) = self.services.cache_dir.as_deref().filter(|d| !d.is_empty()) else {
            return;
        };
        if !dir.starts_with('/') {
            errors.push(format!(
                "services.cache_dir must be an absolute path, got: {dir}"
            ));
        } else if is_cloud && !dir.starts_with("/app") {
            errors.push(format!(
                "Cloud profile services.cache_dir should start with /app, got: {dir}"
            ));
        }
    }

    pub(crate) fn validate_secrets(&self, errors: &mut Vec<String>) {
        let Some(secrets) = self.secrets.as_ref() else {
            return;
        };
        if let Err(e) = secrets.validate() {
            errors.push(e.to_string());
        }
        let Some(vault) = secrets.vault.as_ref() else {
            return;
        };

        let trusted = trusted_http_hosts_from_env();
        if let Err(e) = validate_outbound_url_with_trust(&vault.address, &trusted) {
            errors.push(format!("secrets.vault.address is not reachable: {e}"));
        }
        if vault.path.trim().is_empty() {
            errors.push("secrets.vault.path is required".to_owned());
        }
        if vault.mount.trim().is_empty() {
            errors.push("secrets.vault.mount must not be empty".to_owned());
        }
        if vault.timeout_secs == 0 || vault.timeout_secs > MAX_VAULT_TIMEOUT_SECS {
            errors.push(format!(
                "secrets.vault.timeout_secs must be between 1 and {MAX_VAULT_TIMEOUT_SECS}, got \
                 {}",
                vault.timeout_secs
            ));
        }
        if vault.retries > MAX_VAULT_RETRIES {
            errors.push(format!(
                "secrets.vault.retries must be at most {MAX_VAULT_RETRIES}, got {}",
                vault.retries
            ));
        }
        if let VaultAuth::Kubernetes { role, .. } = &vault.auth
            && role.trim().is_empty()
        {
            errors.push("secrets.vault.auth.role is required for the kubernetes method".to_owned());
        }
        for (name, key) in &vault.keys {
            if key.path.trim().is_empty() || key.field.trim().is_empty() {
                errors.push(format!(
                    "secrets.vault.keys.{name} requires a non-empty path and field"
                ));
            }
        }
    }
}

fn validate_verification(label: &str, verify: &BundleVerification, errors: &mut Vec<String>) {
    let has_digest = verify.sha256.is_some();
    let has_keys = !verify.ed25519_public_keys.is_empty();

    if has_digest == has_keys {
        errors.push(format!(
            "{label}.verify requires exactly one of sha256 or ed25519_public_keys; a fetched \
             services bundle is never trusted unverified"
        ));
    }

    if let Some(digest) = verify.sha256.as_deref() {
        let lower_hex = digest
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c));
        if digest.len() != 64 || !lower_hex {
            errors.push(format!(
                "{label}.verify.sha256 must be 64 lowercase hex characters, got: {digest}"
            ));
        }
    }

    for (idx, key) in verify.ed25519_public_keys.iter().enumerate() {
        match base64::engine::general_purpose::STANDARD.decode(key) {
            Ok(bytes) if bytes.len() == ED25519_PUBLIC_KEY_LEN => {},
            Ok(bytes) => errors.push(format!(
                "{label}.verify.ed25519_public_keys[{idx}] decodes to {} bytes, expected \
                 {ED25519_PUBLIC_KEY_LEN}",
                bytes.len()
            )),
            Err(e) => errors.push(format!(
                "{label}.verify.ed25519_public_keys[{idx}] is not valid base64: {e}"
            )),
        }
    }
}
