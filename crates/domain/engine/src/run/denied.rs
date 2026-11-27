//! How a tool call that never ran, or was cut short, is marked on its stored
//! part, so a reader need not parse the error text.
//!
//! A call refused before or while it ran ends as `ToolState::Error` whose
//! `metadata` holds [`KEY`](crate::denied::KEY) naming who stopped it:
//! [`POLICY`](crate::denied::POLICY) (the Governor's verdict, its text under
//! [`REASON`](crate::denied::REASON)), [`RULE`](crate::denied::RULE) (a
//! permission rule that denies it, the rule's text under
//! [`REASON`](crate::denied::REASON)), [`REJECTED`](crate::denied::REJECTED)
//! (the user, with their words under [`FEEDBACK`](crate::denied::FEEDBACK) when
//! they gave any) or [`ABORTED`](crate::denied::ABORTED) (the run was
//! aborted while the call waited or ran). A call that failed for any other
//! reason carries none of these keys.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use harness_store::JsonMap;
use serde_json::Value;

pub const KEY: &str = "denied";
pub const POLICY: &str = "policy";
pub const RULE: &str = "rule";
pub const REJECTED: &str = "rejected";
pub const ABORTED: &str = "aborted";
pub const REASON: &str = "reason";
pub const FEEDBACK: &str = "feedback";

pub(crate) fn marker(kind: &str, detail: Option<(&str, &str)>) -> JsonMap {
    let mut metadata = JsonMap::new();
    metadata.insert(KEY.to_owned(), Value::String(kind.to_owned()));
    if let Some((key, text)) = detail {
        metadata.insert(key.to_owned(), Value::String(text.to_owned()));
    }
    metadata
}
