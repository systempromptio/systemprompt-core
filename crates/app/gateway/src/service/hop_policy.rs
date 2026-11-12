//! What the gateway may do to a request on the hop it is bound to.
//!
//! [`HopPolicy`] is derived once from the bound hop's credential and carried
//! on the resolved upstream and the outbound context. A governed hop is the
//! deployment's own account with the upstream: the gateway shapes the request,
//! repairs it, retries it and joins it to its trace. A hop served as presented
//! is the member's own account: the request reaches the upstream as the client
//! built it, once, and the answer returns whole; whatever a governed hop would
//! have repaired is refused instead. Every stage that changes, re-sends or
//! annotates a request asks one of these methods, so a stage that asks none
//! has no decision to get wrong.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_manifest::services::catalog::ResolvedHop;

use super::stages::PromptRepair;
use crate::protocol::outbound::retry::RetryPolicy;

/// The treatment a bound hop's request and response receive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HopPolicy {
    Governed,
    AsPresented,
}

impl HopPolicy {
    #[must_use]
    pub const fn of(hop: &ResolvedHop) -> Self {
        if hop.credential.is_caller() {
            Self::AsPresented
        } else {
            Self::Governed
        }
    }

    const fn governed(self) -> bool {
        matches!(self, Self::Governed)
    }

    #[must_use]
    pub const fn rewrites_request(self) -> bool {
        self.governed()
    }

    #[must_use]
    pub const fn repairs_prompt(self) -> PromptRepair {
        match self {
            Self::Governed => PromptRepair::Rewrite,
            Self::AsPresented => PromptRepair::Refuse,
        }
    }

    #[must_use]
    pub const fn redacts(self) -> bool {
        self.governed()
    }

    #[must_use]
    pub const fn retries_upstream(self) -> bool {
        self.governed()
    }

    #[must_use]
    pub fn retry_policy(self, ambient: RetryPolicy) -> RetryPolicy {
        if self.retries_upstream() {
            ambient
        } else {
            RetryPolicy::none()
        }
    }

    #[must_use]
    pub const fn learns_refusals(self) -> bool {
        self.governed()
    }

    #[must_use]
    pub const fn stamps_trace(self) -> bool {
        self.governed()
    }

    #[must_use]
    pub const fn returns_response_whole(self) -> bool {
        !self.governed()
    }
}
