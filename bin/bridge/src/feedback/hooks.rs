//! Device authentication stays in protected enrollment storage, never authored
//! hooks: the proxy strips whatever a hook sent and re-attaches the enrolled
//! credential and host on the way upstream. The `x-systemprompt-host` stamp in
//! authored hooks is applied per host copy by the sync emitters.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::credentials::Enrollment;
use super::{FeedbackError, Result};

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
