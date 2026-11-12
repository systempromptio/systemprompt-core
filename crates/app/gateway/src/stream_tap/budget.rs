//! The ceiling on how much response content a tapped stream accumulates.
//!
//! The tap folds every text, thinking and tool-call delta into the snapshot
//! that terminal frames, the audit row and the response safety scan read. A
//! snapshot that silently dropped content would hand each of them a partial
//! response as if it were whole, so a stream whose content passes
//! [`MAX_ACCUMULATED_RESPONSE_BYTES`] ends as a failure carrying
//! [`ACCUMULATION_CAP_MESSAGE`]: the client gets an error frame, the audit
//! row is failed and settles the partial usage, and no consumer ever reads a
//! truncated snapshot as a complete one. The cap matches the buffered-body cap,
//! so a response is held to one size whichever way it arrives.
//!
//! A pass-through tap charges the bytes it releases instead of the decoded
//! deltas ([`ContentBudget::forwarded_bytes`]), so a frame the codec has no
//! event for still counts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::protocol::canonical::CanonicalEvent;
use crate::protocol::outbound::UpstreamBounds;

pub const MAX_ACCUMULATED_RESPONSE_BYTES: usize = UpstreamBounds::PRODUCTION.max_buffered_bytes;

pub const ACCUMULATION_CAP_MESSAGE: &str =
    "upstream response exceeds the gateway's response size cap";

/// Content bytes a tap has accumulated, and whether the cap was crossed.
#[derive(Debug, Clone, Copy, Default)]
pub struct ContentBudget {
    accumulated: usize,
    exceeded: bool,
    forwarded_bytes: bool,
}

impl ContentBudget {
    #[must_use]
    pub const fn forwarded_bytes() -> Self {
        Self {
            accumulated: 0,
            exceeded: false,
            forwarded_bytes: true,
        }
    }

    pub const fn admit(&mut self, event: &CanonicalEvent) -> bool {
        if self.exceeded {
            return false;
        }
        if self.forwarded_bytes {
            return true;
        }
        let len = match event {
            CanonicalEvent::TextDelta { text, .. } | CanonicalEvent::ThinkingDelta { text, .. } => {
                text.len()
            },
            CanonicalEvent::ToolUseDelta { partial_json, .. } => partial_json.len(),
            _ => 0,
        };
        self.charge(len)
    }

    pub const fn admit_bytes(&mut self, len: usize) -> bool {
        if self.exceeded {
            return false;
        }
        self.charge(len)
    }

    const fn charge(&mut self, len: usize) -> bool {
        let next = self.accumulated.saturating_add(len);
        if next > MAX_ACCUMULATED_RESPONSE_BYTES {
            self.exceeded = true;
            return false;
        }
        self.accumulated = next;
        true
    }

    #[must_use]
    pub const fn exceeded(&self) -> bool {
        self.exceeded
    }
}
