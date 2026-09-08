//! Lifecycle status and purpose of a persisted AI request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestStatus {
    Pending,
    Completed,
    Failed,
    Rejected,
}

impl RequestStatus {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Rejected => "rejected",
        }
    }
}

/// What a request was for.
///
/// A `probe` never carries a reply worth reading: clients send `max_tokens: 1`
/// calls to count tokens or warm a prompt cache. `utility` is reserved for a
/// consumer that can tell a title-generation side call from a real turn; the
/// gateway itself only distinguishes probes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RequestKind {
    #[default]
    Turn,
    Probe,
    Utility,
}

impl RequestKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Turn => "turn",
            Self::Probe => "probe",
            Self::Utility => "utility",
        }
    }

    pub const fn classify(max_tokens: Option<u32>) -> Self {
        match max_tokens {
            Some(n) if n <= 1 => Self::Probe,
            _ => Self::Turn,
        }
    }
}
