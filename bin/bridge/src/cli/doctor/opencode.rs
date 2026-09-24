//! `OpenCode` admin-tier drift: whether the managed `opencode.json` still
//! declares the model list the gateway serves today.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::path::Path;

use super::Check;
use crate::context::BridgeContext;
use crate::integration::opencode::{OPENCODE_HOST, admin_tier_models};
use crate::integration::reapply::{ModelProtocolOverrides, build_profile_inputs};

const NAME: &str = "opencode admin models";

pub async fn check_admin_tier_models(bridge: &BridgeContext) -> Option<Check> {
    let (path, installed) = admin_tier_models()?;
    let overrides = ModelProtocolOverrides::new();
    let inputs = match build_profile_inputs(bridge, &OPENCODE_HOST, &overrides).await {
        Ok(inputs) => inputs,
        Err(e) => {
            return Some(Check::warn(
                NAME,
                format!(
                    "{}: cannot compare with the gateway catalogue: {e}",
                    path.display()
                ),
            ));
        },
    };
    Some(check_model_drift(&path, &installed, &inputs.models))
}

pub fn check_model_drift(path: &Path, installed: &[String], expected: &[String]) -> Check {
    let installed: BTreeSet<&str> = installed.iter().map(String::as_str).collect();
    let expected: BTreeSet<&str> = expected.iter().map(String::as_str).collect();
    let retired: Vec<&str> = installed.difference(&expected).copied().collect();
    let missing: Vec<&str> = expected.difference(&installed).copied().collect();
    if retired.is_empty() && missing.is_empty() {
        return Check::ok(
            NAME,
            format!("{}: matches the gateway catalogue", path.display()),
        );
    }
    let mut parts = Vec::new();
    if !retired.is_empty() {
        parts.push(format!("no longer served: {}", retired.join(", ")));
    }
    if !missing.is_empty() {
        parts.push(format!("not listed: {}", missing.join(", ")));
    }
    Check::warn(
        NAME,
        format!(
            "{}: model list differs from the gateway catalogue ({}); re-run install --host \
             opencode as administrator to refresh it",
            path.display(),
            parts.join("; ")
        ),
    )
}
