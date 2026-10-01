//! Identifiers the bridge binary carries between coding-agent hosts and the
//! gateway: the MCP transport session a probe negotiated, the hook session a
//! host reports, the id of a comms announcement, and the Claude Desktop
//! deployment organisation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::IdValidationError;

crate::define_id!(McpSessionId, checked, |value| {
    crate::macros::validate_non_empty("McpSessionId", value)
});
crate::define_id!(HookSessionId, checked, |value| {
    crate::macros::validate_non_empty("HookSessionId", value)
});
crate::define_id!(CommsMessageId, checked, |value| {
    crate::macros::validate_non_empty("CommsMessageId", value)
});
crate::define_id!(
    DeploymentOrganizationUuid,
    checked,
    validate_deployment_organization_uuid
);

fn validate_deployment_organization_uuid(value: &str) -> Result<(), IdValidationError> {
    let hyphenated = value.len() == 36 && value.bytes().filter(|&b| b == b'-').count() == 4;
    if hyphenated && uuid::Uuid::try_parse(value).is_ok() {
        return Ok(());
    }
    Err(IdValidationError::invalid(
        "DeploymentOrganizationUuid",
        "expected a hyphenated UUID",
    ))
}
