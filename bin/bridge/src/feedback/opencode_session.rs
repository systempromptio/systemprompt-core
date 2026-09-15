//! Maps an `OpenCode` native session id onto the UUID session identity the
//! gateway keys contexts on.
//!
//! `OpenCode` sessions are `ses_…` strings, but the gateway lands a request on
//! the same context as its hook events only through `metadata.user_id`, which
//! must carry a UUID. A v5 UUID under a fixed namespace gives the plugin (in
//! JavaScript, over `WebCrypto`) and the proxy (here) the same answer for the
//! same native id without either having to share state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ClientSessionId;
use systemprompt_identifiers::error::IdValidationError;

/// Fixed namespace for v5 session UUIDs. Mirrored verbatim into the emitted
/// `OpenCode` plugin, so it must never change.
// Why: mirrored verbatim into the emitted OpenCode plugin, which derives the
// same v5 UUID; changing it would split every existing session.
pub const OPENCODE_SESSION_NAMESPACE: uuid::Uuid =
    uuid::uuid!("7c1f5b6e-3a2d-4e8f-9b0c-2d6a1e4f8c73");

/// The gateway-facing session id for an `OpenCode` native session id.
///
/// Deterministic: the same native id always maps to the same UUID. The
/// `Result` only exists because the id type validates on construction; a
/// hyphenated v5 UUID always passes.
pub fn session_uuid(native: &str) -> Result<ClientSessionId, IdValidationError> {
    let id = uuid::Uuid::new_v5(&OPENCODE_SESSION_NAMESPACE, native.as_bytes());
    ClientSessionId::try_new(id.hyphenated().to_string())
}
