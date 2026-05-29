//! The AI gateway: a protocol-translating proxy in front of upstream LLM
//! providers.
//!
//! Inbound requests in one wire protocol (Anthropic Messages, `OpenAI`
//! Responses) are parsed into a canonical form, dispatched to an upstream
//! provider via the [`protocol`] adapters, and rendered back in the caller's
//! protocol. [`GatewayService`] orchestrates the flow; supporting modules cover
//! [`policy`] resolution, [`quota`] enforcement, [`safety`] scanning, usage
//! [`captures`], [`pricing`], the upstream [`registry`], and the [`audit`]
//! trail.

pub mod audit;
pub mod captures;
pub mod parse;
pub mod policy;
pub mod pricing;
pub mod protocol;
pub mod quota;
pub mod registry;
pub mod safety;
pub mod service;
pub mod stream_tap;

pub use audit::{GatewayAudit, GatewayRequestContext};
pub use captures::{CapturedToolUse, CapturedUsage};
pub use protocol::{
    CanonicalEvent, CanonicalRequest, CanonicalResponse, InboundAdapter, OutboundAdapter,
    OutboundAdapterRegistration, OutboundCtx, OutboundOutcome,
};
pub use registry::GatewayUpstreamRegistry;
pub use service::{DispatchInputs, GatewayService, REQUEST_ID_HEADER};
