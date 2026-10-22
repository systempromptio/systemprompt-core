//! The caller's own session identifier, carried inside `metadata.user_id`.
//!
//! Claude Code stamps every `/v1/messages` call with
//! `metadata.user_id = "user_<sha256>_account_<uuid>_session_<uuid>"`, and the
//! trailing UUID is the session id it also reports through its hook events.
//! Parsing it lets the gateway land a request on the same context the hooks
//! pipeline writes, without the caller having to send a dedicated header.
//!
//! The value is opaque: a bounded, printable, whitespace-free string, so a
//! harness's own id (an `x-session-id` such as `ses_…` from `OpenCode`) is
//! recorded as is.
//!
//! Distinct from [`crate::SessionId`]: that is the gateway's own attested
//! `sess_` session, minted once per credential and shared by every Claude Code
//! run that credential drives.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::IdValidationError;

const SESSION_SEGMENT: &str = "_session_";

const MAX_LEN: usize = 128;

fn validate(value: &str) -> Result<(), IdValidationError> {
    if value.is_empty() || value.len() > MAX_LEN {
        return Err(IdValidationError::invalid(
            "ClientSessionId",
            "must be 1 to 128 characters",
        ));
    }
    if !value.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(IdValidationError::invalid(
            "ClientSessionId",
            "must be printable ASCII with no whitespace",
        ));
    }
    Ok(())
}

crate::define_id!(ClientSessionId, validated, schema, validate);

#[derive(serde::Deserialize)]
struct MetadataUserId {
    #[serde(rename = "session_id")]
    raw_session: String,
}

impl ClientSessionId {
    pub fn from_metadata_user_id(value: &str) -> Result<Option<Self>, IdValidationError> {
        let value = value.trim();
        let session = if value.starts_with('{') {
            let metadata: MetadataUserId = serde_json::from_str(value)
                .map_err(|e| IdValidationError::json("ClientSessionId", e))?;
            Some(metadata.raw_session)
        } else {
            value
                .rsplit_once(SESSION_SEGMENT)
                .map(|(_, suffix)| suffix.to_owned())
        };
        session
            .map(|value| {
                let parsed = uuid::Uuid::parse_str(value.trim())
                    .map_err(|e| IdValidationError::uuid("ClientSessionId", e))?;
                Self::try_new(parsed.hyphenated().to_string())
            })
            .transpose()
    }
}
