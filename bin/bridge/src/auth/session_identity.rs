//! The session id the bridge presents to the gateway.
//!
//! Every gateway call used to mint a fresh `SessionId::generate()`. The server
//! adopts an unknown id rather than rejecting it, so each hourly token refresh
//! through the credential helper created another `user_sessions` row: 24 rows
//! per user per day on the 2026-09-22 customer instance, 1,070 bridge sessions
//! for 20 users.
//!
//! The id is instead derived from the credential binding this install already
//! computes for its token cache, plus the UTC date. That makes it stable for
//! every process on one machine talking to one gateway with one credential,
//! so a refresh renews the session it already owns, and it rotates when the
//! credential, the gateway or the day changes — each of which genuinely is a
//! new session.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;

use systemprompt_identifiers::SessionId;
use uuid::Uuid;

use crate::config;

use super::cache::CredentialBinding;

/// The stable session id for this install, gateway, credential and day.
///
/// Falls back to a generated id when no credential identity is configured yet
/// — sign-in has nothing to bind to, and one extra session there is correct.
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
