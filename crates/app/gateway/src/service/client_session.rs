//! The `x-opencode-session` header a gateway session presents upstream.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

const OPENCODE_SESSION_HEADER: &str = "x-opencode-session";

// Why: OpenCode Go meters a subscription per `x-opencode-session`, and a
// client that sends none (the bridge folds it into the chat body) would read
// as one session per request. The derived value is stable per client session
// (the harness's own id when it sent one, else the gateway session) and
// reaches only a provider whose `forward_headers` lists the header.
pub(super) fn with_client_session(
    mut forward: Vec<(String, String)>,
    metering_key: Option<&str>,
) -> Vec<(String, String)> {
    let present = forward
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case(OPENCODE_SESSION_HEADER));
    if let (false, Some(session)) = (present, metering_key) {
        let digest = crate::audit::payload::digest_hex(session.as_bytes());
        let derived = format!("ses_{}", digest.get(..26).unwrap_or(&digest));
        forward.push((OPENCODE_SESSION_HEADER.to_owned(), derived));
    }
    forward
}
