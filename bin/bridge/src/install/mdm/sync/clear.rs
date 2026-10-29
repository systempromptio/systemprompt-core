//! Clearing the bridge's Claude Desktop policy on Windows (registry hives)
//! and macOS (managed preferences) when the host is disabled.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(target_os = "windows")]
pub(super) fn clear_bridge_policy(
    mcp: &super::super::MdmPayloadInputs<'_>,
) -> Result<Vec<String>, crate::host_sync::ApplyError> {
    use super::super::clear_plan::{ClearPlan, ObservedPolicy, plan_clear};
    use crate::config::store::{PolicyHive, PolicyTarget};

    let ours = mcp.loopback.secret_fingerprint();
    let store = mcp.policy_store.backend();
    let mut lines = Vec::new();
    for hive in [PolicyHive::User, PolicyHive::Machine] {
        let doc = store
            .read_policy_document(
                hive,
                PolicyTarget::Claude,
                super::super::policy::WRITTEN_POLICY_KEYS,
            )
            .map_err(|e| step("read the Claude Desktop policy", e))?;
        let observed = ObservedPolicy {
            keys: doc.keys().cloned().collect(),
            api_key_fingerprint: doc
                .get(crate::cowork_compat::POLICY_API_KEY)
                .and_then(crate::config::store::PolicyDocumentValue::as_str)
                .map(|v| crate::proxy::secret::fingerprint(v.trim())),
            base_url: doc
                .get("inferenceGatewayBaseUrl")
                .and_then(crate::config::store::PolicyDocumentValue::as_str)
                .map(str::to_owned),
            profile_managed: false,
        };
        let names: Vec<&str> = match plan_clear(Some(&observed), ours.as_deref()) {
            ClearPlan::NothingWritten => continue,
            ClearPlan::StaleBridgePolicy => {
                lines.push(format!(
                    "{}: a previous bridge install's policy; replace it from the app to release it",
                    hive.label()
                ));
                continue;
            },
            ClearPlan::ManagedElsewhere => {
                lines.push(format!(
                    "{}: managed by an administrator; left in place",
                    hive.label()
                ));
                continue;
            },
            ClearPlan::RemoveAll => observed.keys.iter().map(String::as_str).collect(),
            ClearPlan::RemoveKeys(keys) => keys,
        };
        if hive == PolicyHive::Machine && !crate::winproc::is_elevated() {
            return Err(crate::host_sync::ApplyError::ElevationRequired {
                what: "the Claude Desktop machine policy",
                detail: format!("removing {} bridge value(s) from HKLM", names.len()),
            });
        }
        let removed = crate::config::store::verified::remove_values(
            store,
            hive,
            PolicyTarget::Claude,
            &names,
        )
        .map_err(|e| step("remove the bridge's Claude Desktop policy", e))?;
        lines.push(format!(
            "{}: removed {removed} bridge value(s)",
            hive.label()
        ));
    }
    Ok(lines)
}

#[cfg(target_os = "macos")]
pub(super) fn clear_bridge_policy(
    mcp: &super::super::MdmPayloadInputs<'_>,
) -> Result<Vec<String>, crate::host_sync::ApplyError> {
    super::super::macos_remove::clear_bridge_preferences(
        mcp.elevation_gate,
        mcp.loopback.secret_fingerprint().as_deref(),
    )
    .map_err(|e| step("remove the bridge's Claude Desktop managed preferences", e))
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn step(
    context: &'static str,
    e: impl std::error::Error + Send + Sync + 'static,
) -> crate::host_sync::ApplyError {
    crate::host_sync::ApplyError::Step {
        context,
        source: Box::new(e),
    }
}
