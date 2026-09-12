//! The caller's own session identifier, carried inside `metadata.user_id`.
//!
//! Claude Code stamps every `/v1/messages` call with
//! `metadata.user_id = "user_<sha256>_account_<uuid>_session_<uuid>"`, and the
//! trailing UUID is the session id it also reports through its hook events.
//! Parsing it lets the gateway land a request on the same context the hooks
//! pipeline writes, without the caller having to send a dedicated header.
//!
//! Distinct from [`crate::SessionId`]: that is the gateway's own attested
//! `sess_` session, minted once per credential and shared by every Claude Code
//! run that credential drives.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::IdValidationError;

const SESSION_SEGMENT: &str = "_session_";

fn validate(value: &str) -> Result<(), IdValidationError> {
    let parsed = uuid::Uuid::parse_str(value)
        .map_err(|e| IdValidationError::invalid("ClientSessionId", e.to_string()))?;
    if parsed.hyphenated().to_string() != value {
        return Err(IdValidationError::invalid(
            "ClientSessionId",
            "must be a lowercase hyphenated UUID",
        ));
    }
    Ok(())
}

crate::define_id!(ClientSessionId, validated, schema, validate);

impl ClientSessionId {
    pub fn from_metadata_user_id(value: &str) -> Result<Option<Self>, IdValidationError> {
        let value = value.trim();
        let session = if value.starts_with('{') {
            let metadata: serde_json::Value = serde_json::from_str(value)
                .map_err(|e| IdValidationError::invalid("ClientSessionId", e.to_string()))?;
            Some(metadata.get("session_id").and_then(serde_json::Value::as_str)
                .ok_or_else(|| IdValidationError::invalid("ClientSessionId", "metadata requires a string session_id"))?.to_owned())
        } else {
            value.rsplit_once(SESSION_SEGMENT).map(|(_, suffix)| suffix.to_owned())
        };
        session.map(|value| {
            let parsed = uuid::Uuid::parse_str(value.trim())
                .map_err(|e| IdValidationError::invalid("ClientSessionId", e.to_string()))?;
            Self::try_new(parsed.hyphenated().to_string())
        }).transpose()
    }
}
