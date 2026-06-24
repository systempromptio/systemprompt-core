//! Slack request-signature verification.
//!
//! Slack signs every request with `X-Slack-Signature: v0=<hex>` where the MAC
//! is `HMAC-SHA256(signing_secret, "v0:{timestamp}:{raw_body}")`. Verification
//! must run over the **exact** received body before any deserialization, and
//! must reject replayed requests whose timestamp drifts beyond a tolerance
//! window. The comparison is constant-time (delegated to `Mac::verify_slice`).

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

use crate::error::{SlackError, SlackResult};

type HmacSha256 = Hmac<Sha256>;

/// Maximum accepted drift between the request timestamp and local time.
pub const MAX_TIMESTAMP_SKEW_SECS: i64 = 60 * 5;

/// Verify a Slack signature against the raw request body.
///
/// `timestamp` is the `X-Slack-Request-Timestamp` header (unix seconds as
/// sent), `signature` is the `X-Slack-Signature` header (`v0=...`), and
/// `now_unix` is the current unix time — injected so the check is testable.
pub fn verify_slack_signature(
    signing_secret: &[u8],
    timestamp: &str,
    signature: &str,
    body: &[u8],
    now_unix: i64,
) -> SlackResult<()> {
    let ts: i64 = timestamp.parse().map_err(|e| {
        SlackError::MalformedRequest(format!("invalid X-Slack-Request-Timestamp: {e}"))
    })?;
    if (now_unix - ts).abs() > MAX_TIMESTAMP_SKEW_SECS {
        return Err(SlackError::StaleTimestamp);
    }

    let provided = signature
        .strip_prefix("v0=")
        .ok_or_else(|| SlackError::Signature("missing v0= prefix".to_owned()))?;
    let provided = hex::decode(provided)
        .map_err(|e| SlackError::Signature(format!("signature is not valid hex: {e}")))?;

    let mut mac = HmacSha256::new_from_slice(signing_secret)
        .map_err(|e| SlackError::Internal(e.to_string()))?;
    mac.update(b"v0:");
    mac.update(timestamp.as_bytes());
    mac.update(b":");
    mac.update(body);

    mac.verify_slice(&provided)
        .map_err(|e| SlackError::Signature(format!("HMAC mismatch: {e}")))
}

/// Compute the `v0=<hex>` signature for a body — used by tests and to mirror
/// Slack's own algorithm in fixtures.
#[must_use]
#[expect(
    clippy::expect_used,
    reason = "HMAC-SHA256 accepts any key length by construction; new_from_slice cannot fail here"
)]
pub fn sign(signing_secret: &[u8], timestamp: &str, body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(signing_secret).expect("HMAC accepts any key length");
    mac.update(b"v0:");
    mac.update(timestamp.as_bytes());
    mac.update(b":");
    mac.update(body);
    format!("v0={}", hex::encode(mac.finalize().into_bytes()))
}
