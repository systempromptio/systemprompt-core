//! Coalesce keys: `<kind>:<stream>:<primary kind>:<primary
//! id>:<actor>:<bucket>`.
//!
//! `-` stands for an absent stream or subject, and the bucket is the event
//! time divided by the coalescing window. Two actors never share a key, so a
//! fold never mixes whose event the row announces. Without a window the bucket
//! is the outbox id, so every event keeps its own row. A digest holds a row
//! until the end of the digest window its event falls in.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::EventOutboxId;
use systemprompt_models::notifications::{DigestWindow, NotificationEvent};

#[must_use]
pub fn coalesce_key(
    event: &NotificationEvent,
    window_secs: Option<u32>,
    outbox_id: &EventOutboxId,
) -> String {
    let (kind, id) = event
        .subjects
        .first()
        .map_or(("-", "-"), |s| (s.kind.as_str(), s.id.as_str()));
    let bucket = match window_secs {
        Some(window) if window > 0 => {
            (event.occurred_at.timestamp() / i64::from(window)).to_string()
        },
        _ => outbox_id.as_str().to_owned(),
    };
    let stream = event.stream.as_ref().map_or("-", |s| s.as_str());
    format!(
        "{}:{stream}:{kind}:{id}:{}:{bucket}",
        event.kind, event.actor
    )
}

#[must_use]
pub fn digest_release(
    event: &NotificationEvent,
    digest: Option<DigestWindow>,
) -> Option<DateTime<Utc>> {
    let every = i64::from(digest?.every_secs);
    if every == 0 {
        return None;
    }
    let end = (event.occurred_at.timestamp().div_euclid(every) + 1) * every;
    DateTime::from_timestamp(end, 0)
}
