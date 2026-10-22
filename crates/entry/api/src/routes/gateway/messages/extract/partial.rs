//! What the gateway knows about a request at the moment it is rejected.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
use systemprompt_identifiers::{
    ClientSessionId, ContextId, GatewayConversationId, SessionId, TraceId, UserId,
};
use systemprompt_models::attribution::RequestAttribution;
use systemprompt_models::origin::{ClientEvidence, RequestOrigin};

/// What is known about a request at the moment it is rejected.
///
/// `origin` is fixed at entry from the route and `User-Agent`, so a rejection
/// row is never persisted without its client and wire protocol; `evidence` is
/// filled once the attribution headers and body have been classified.
#[derive(Debug)]
pub struct RejectionPartial {
    pub origin: RequestOrigin,
    pub evidence: Option<ClientEvidence>,
    pub attribution: RequestAttribution,
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub context_id: Option<ContextId>,
    pub gateway_conversation_id: Option<GatewayConversationId>,
    pub client_session_id: Option<ClientSessionId>,
    pub trace_id: Option<TraceId>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub max_tokens: Option<u32>,
    pub is_streaming: bool,
    pub body: Option<Bytes>,
}

impl RejectionPartial {
    pub const fn new(origin: RequestOrigin) -> Self {
        Self {
            origin,
            evidence: None,
            attribution: RequestAttribution {
                entries: Vec::new(),
                api_key_id: None,
            },
            user_id: None,
            session_id: None,
            context_id: None,
            gateway_conversation_id: None,
            client_session_id: None,
            trace_id: None,
            provider: None,
            model: None,
            max_tokens: None,
            is_streaming: false,
            body: None,
        }
    }
}
