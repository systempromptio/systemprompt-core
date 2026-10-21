//! The session id the bridge presents to the gateway.
//!
//! The server adopts an unknown session id rather than rejecting it, so a
//! bridge that minted a fresh `SessionId::generate()` per call would create a
//! `user_sessions` row on every hourly token refresh through the credential
//! helper.
//!
//! The id is instead derived from the credential binding this install already
//! computes for its token cache, plus the UTC date. That makes it stable for
//! every process on one machine talking to one gateway with one credential,
//! so a refresh renews the session it already owns, and it rotates when the
//! credential, the gateway or the day changes — each of which genuinely is a
//! new session. With no credential identity configured yet, sign-in has
//! nothing to bind to and a generated id is correct.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;

use systemprompt_identifiers::SessionId;
use uuid::Uuid;

use crate::config;

use super::cache::CredentialBinding;

#[must_use]
pub fn stable_session_id(cfg: &config::Config) -> SessionId {
    derive(cfg).unwrap_or_else(|_| SessionId::generate())
}

fn derive(cfg: &config::Config) -> io::Result<SessionId> {
    let binding = CredentialBinding::capture(cfg)?;
    let day = chrono::Utc::now().date_naive();
    let seed = format!("{}|{}|{day}", binding.gateway(), binding.digest());
    let uuid = Uuid::new_v5(&Uuid::NAMESPACE_OID, seed.as_bytes());
    Ok(SessionId::new(format!("sess_{}", uuid.hyphenated())))
}
