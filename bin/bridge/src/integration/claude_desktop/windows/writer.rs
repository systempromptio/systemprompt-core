//! Delegated Claude Desktop policy installation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::require_org_plugins_provisioned;
use crate::integration::host_app::ProfileInstalled;

pub(super) fn install_through_writer(
    entries: &[(String, String)],
) -> Result<Option<ProfileInstalled>, crate::install::policy_writer::PolicyWriterError> {
    use crate::install::policy_writer::{self, PolicyWriterError, WriterStatus};
    match policy_writer::status() {
        WriterStatus::Ready => {},
        WriterStatus::NotRegistered => return Ok(None),
        WriterStatus::Unavailable(why) => return Err(PolicyWriterError::Unavailable(why)),
    }
    let Some(fragment) =
        crate::mcp_registry::read_envelope().map_err(|source| PolicyWriterError::Io {
            context: "read the last verified manifest envelope".to_owned(),
            source,
        })?
    else {
        return Err(PolicyWriterError::Unavailable(
            "no verified manifest envelope has been kept yet; sync once first".to_owned(),
        ));
    };
    let catalog =
        crate::install::mdm::tool_catalog::read().map_err(|source| PolicyWriterError::Io {
            context: "read the tool catalog".to_owned(),
            source,
        })?;
    let requester = crate::windows_acl::current_sid().map_err(|source| PolicyWriterError::Io {
        context: "resolve the requesting account".to_owned(),
        source,
    })?;
    let loopback =
        policy_writer::Loopback::from_entries(entries).map_err(|source| PolicyWriterError::Io {
            context: "read the proxy from the staged profile".to_owned(),
            source,
        })?;
    let request = policy_writer::build_request(
        loopback,
        &fragment,
        catalog,
        policy_writer::facts_from_entries(entries),
        requester,
    );
    policy_writer::write_policy(&request)?;
    let outcome = require_org_plugins_provisioned(false).map_err(|e| {
        PolicyWriterError::Unavailable(format!(
            "policy written through the elevated writer, but org-plugins is not usable: {e}"
        ))
    })?;
    tracing::info!(
        value_count = entries.len(),
        "Claude Desktop profile installed through the elevated policy writer"
    );
    Ok(Some(outcome))
}
