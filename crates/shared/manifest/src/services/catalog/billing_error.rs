//! Catalog shapes refused because they would move a request onto a bill
//! nobody chose, or rewrite a request that must arrive as its client built it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{CredentialId, EndpointId, ModelId, ProviderId, RouteId};
use systemprompt_wire::WireProtocol;
use thiserror::Error;

/// A chain or endpoint whose billing or wire a member-paid credential forbids.
#[derive(Debug, Error)]
pub enum BillingRuleError {
    #[error(
        "caller credential '{credential}' is used by endpoint '{provider}/{endpoint}' on wire \
         '{wire}', which no client request format speaks; a credential the client presents is \
         served only on the wire the client itself sent"
    )]
    CallerWireRewritesRequest {
        credential: CredentialId,
        provider: ProviderId,
        endpoint: EndpointId,
        wire: WireProtocol,
    },

    #[error(
        "model '{model}' is deployed on '{first}' (credential '{first_credential}') and \
         '{second}' (credential '{second_credential}'); a model with a member-paid deployment \
         bills one credential on every deployment"
    )]
    MixedChain {
        model: ModelId,
        first: String,
        first_credential: CredentialId,
        second: String,
        second_credential: CredentialId,
    },

    #[error(
        "route '{route}' lists context fallback '{fallback}' (credential \
         '{fallback_credential}') for model '{model}' (credential '{credential}'); a fallback \
         of a member-paid model bills the same credential"
    )]
    MixedContextFallback {
        route: RouteId,
        model: ModelId,
        credential: CredentialId,
        fallback: ModelId,
        fallback_credential: CredentialId,
    },
}
