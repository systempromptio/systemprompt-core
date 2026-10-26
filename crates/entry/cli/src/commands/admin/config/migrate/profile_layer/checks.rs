//! Reads of a pre-0.73 profile layer: the stated URL fields, and the checks
//! that the values 0.73 derives equal the ones 0.72 stated.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_yaml::Value;

use super::ProfileMigrationError;
use systemprompt_manifest::profile::derived::{AUTHZ_WEBHOOK_PATH, internal_url_for};

pub(super) fn lookup<'v>(value: &'v Value, path: &[&str]) -> Option<&'v Value> {
    path.iter()
        .try_fold(value, |node, key| node.as_mapping()?.get(*key))
}

pub(super) fn text_of(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

pub(super) fn normalised(url: &str) -> String {
    url.trim().trim_end_matches('/').to_owned()
}

pub(super) fn is_reference(text: &str) -> bool {
    text.contains("${")
}

pub(super) fn agree(
    first: &'static str,
    first_value: &str,
    second: &'static str,
    second_value: &str,
) -> Result<(), ProfileMigrationError> {
    if normalised(first_value) == normalised(second_value) {
        return Ok(());
    }
    Err(ProfileMigrationError::Disagreement {
        first,
        first_value: first_value.to_owned(),
        second,
        second_value: second_value.to_owned(),
    })
}

pub(super) fn origin_of(external: &str) -> Option<String> {
    url::Url::parse(&normalised(external))
        .ok()
        .map(|url| url.origin().ascii_serialization())
}

pub(super) fn stated_external(root: &Value) -> Result<String, ProfileMigrationError> {
    let field = |key: &str| lookup(root, &["server", key]).and_then(text_of);
    if let Some(current) = field("external_url") {
        return Ok(current);
    }
    let external = field("api_external_url");
    let server = field("api_server_url");
    match (external, server) {
        (Some(external), Some(server)) => {
            agree(
                "server.api_external_url",
                &external,
                "server.api_server_url",
                &server,
            )?;
            Ok(external)
        },
        (Some(url), None) | (None, Some(url)) => Ok(url),
        (None, None) => Err(ProfileMigrationError::MissingExternalUrl),
    }
}

pub(super) fn check_issuer(root: &Value, external: &str) -> Result<(), ProfileMigrationError> {
    let Some(issuer) = lookup(root, &["security", "jwt_issuer"]).and_then(text_of) else {
        return Ok(());
    };
    if normalised(&issuer) == normalised(external) {
        return Ok(());
    }
    Err(ProfileMigrationError::IssuerChange {
        issuer,
        external: external.to_owned(),
    })
}

pub(super) fn check_https(root: &Value, external: &str) -> Result<(), ProfileMigrationError> {
    let Some(stated) = lookup(root, &["server", "use_https"]).and_then(Value::as_bool) else {
        return Ok(());
    };
    if is_reference(external) || stated == external.trim().starts_with("https://") {
        return Ok(());
    }
    Err(ProfileMigrationError::Disagreement {
        first: "server.use_https",
        first_value: stated.to_string(),
        second: "server.external_url",
        second_value: external.to_owned(),
    })
}

pub(super) fn derived_internal(root: &Value) -> Option<String> {
    let host = lookup(root, &["server", "host"]).and_then(text_of)?;
    let port = lookup(root, &["server", "port"])
        .and_then(Value::as_u64)
        .and_then(|p| u16::try_from(p).ok())?;
    Some(internal_url_for(&host, port))
}

pub(super) fn check_internal(root: &Value) -> Result<(), ProfileMigrationError> {
    let derived = derived_internal(root);
    if let Some(stated) = lookup(root, &["server", "api_internal_url"]).and_then(text_of)
        && !is_reference(&stated)
        && let Some(derived) = derived.as_ref()
        && normalised(&stated) != normalised(derived)
    {
        return Err(ProfileMigrationError::InternalUrlChange {
            stated,
            derived: derived.clone(),
        });
    }
    let webhook = lookup(root, &["governance", "authz", "hook", "mode"])
        .and_then(Value::as_str)
        .is_some_and(|mode| mode == "webhook");
    if let Some(stated) = lookup(root, &["governance", "authz", "hook", "url"]).and_then(text_of)
        && webhook
        && !is_reference(&stated)
        && let Some(internal) = derived
    {
        let derived = format!("{internal}{AUTHZ_WEBHOOK_PATH}");
        if normalised(&stated) != normalised(&derived) {
            return Err(ProfileMigrationError::HookUrlChange { stated, derived });
        }
    }
    Ok(())
}

pub(super) fn extra_origins(root: &Value, external: &str) -> Vec<String> {
    let own = origin_of(external);
    let mut extra: Vec<String> = lookup(root, &["server", "extra_cors_origins"])
        .and_then(Value::as_sequence)
        .map(|seq| seq.iter().filter_map(text_of).collect())
        .unwrap_or_default();
    let stated = lookup(root, &["server", "cors_allowed_origins"])
        .and_then(Value::as_sequence)
        .map(|seq| seq.iter().filter_map(text_of).collect::<Vec<_>>())
        .unwrap_or_default();
    for origin in stated {
        let is_own = own.as_deref().is_some_and(|own| normalised(&origin) == own)
            || normalised(&origin) == normalised(external);
        if !is_own && !extra.contains(&origin) {
            extra.push(origin);
        }
    }
    extra
}
