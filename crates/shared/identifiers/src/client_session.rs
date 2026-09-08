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
    // Why: the suffix after the last `_session_` is the only part with a
    // stable shape; the prefix segments vary by client and account.
    #[must_use]
    pub fn from_metadata_user_id(user_id: &str) -> Option<Self> {
        let (_, suffix) = user_id.rsplit_once(SESSION_SEGMENT)?;
        let parsed = uuid::Uuid::parse_str(suffix.trim()).ok()?;
        Some(Self::new_unchecked(parsed.hyphenated().to_string()))
    }
}
