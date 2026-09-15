//! Device authentication stays in protected enrollment storage, never authored
//! hooks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::credentials::Enrollment;
use super::{FeedbackError, Result};
use std::path::Path;
use systemprompt_models::feedback::EvaluatorClient;

pub fn stamp_native_hooks(skill_root: &Path, host: EvaluatorClient) -> Result<()> {
    if !matches!(
        host,
        EvaluatorClient::ClaudeCode | EvaluatorClient::ClaudeDesktop
    ) {
        return Ok(());
    }
    let Some(plugin_root) = skill_root.parent().and_then(Path::parent) else {
        return Err(FeedbackError::Readback);
    };
    let path = plugin_root.join("hooks/hooks.json");
    if !path.exists() {
        return Ok(());
    }
    let metadata = std::fs::symlink_metadata(&path)?;
    if !metadata.is_file() || metadata.len() > 1024 * 1024 {
        return Err(FeedbackError::Readback);
    }
    let mut document: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
    let host = serde_json::to_value(host)?;
    if let Some(events) = document
        .get_mut("hooks")
        .and_then(serde_json::Value::as_object_mut)
    {
        for matchers in events
            .values_mut()
            .filter_map(serde_json::Value::as_array_mut)
        {
            for matcher in matchers {
                if let Some(hooks) = matcher
                    .get_mut("hooks")
                    .and_then(serde_json::Value::as_array_mut)
                {
                    for hook in hooks {
                        if hook.get("type").and_then(serde_json::Value::as_str) != Some("http") {
                            continue;
                        }
                        if let Some(headers) = hook
                            .get_mut("headers")
                            .and_then(serde_json::Value::as_object_mut)
                        {
                            headers.insert("x-systemprompt-host".to_owned(), host.clone());
                            headers.remove("x-systemprompt-device-credential");
                        }
                    }
                }
            }
        }
    }
    crate::fsutil::atomic_write_0644(&path, &serde_json::to_vec_pretty(&document)?)?;
    Ok(())
}

pub fn authenticate_forwarded_hook(
    gateway: &str,
    native_host: Option<&str>,
    headers: &mut http::HeaderMap,
) -> Result<()> {
    headers.remove("x-systemprompt-device-credential");
    headers.remove("x-systemprompt-host");
    let Some(host) = native_host.and_then(super::client_kind) else {
        return Ok(());
    };
    let enrollment = Enrollment::load(&super::metadata_root()?, gateway)?;
    let mut credential = http::HeaderValue::from_str(enrollment.credential())?;
    credential.set_sensitive(true);
    let host = serde_json::to_value(host)?
        .as_str()
        .ok_or(FeedbackError::Scope)?
        .to_owned();
    headers.insert("x-systemprompt-device-credential", credential);
    headers.insert("x-systemprompt-host", http::HeaderValue::from_str(&host)?);
    Ok(())
}
