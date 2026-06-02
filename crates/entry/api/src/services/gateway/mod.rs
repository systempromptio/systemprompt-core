//! The AI gateway: a protocol-translating proxy in front of upstream LLM
//! providers.
//!
//! Inbound requests in one wire protocol (Anthropic Messages, `OpenAI`
//! Responses) are parsed into a canonical form, dispatched to an upstream
//! provider via the [`protocol`] adapters, and rendered back in the caller's
//! protocol. [`GatewayService`] orchestrates the flow; supporting modules cover
//! [`policy`] resolution, [`quota`] enforcement, safety scanning, usage
//! [`captures`], [`pricing`], the upstream and safety-scanner [`registry`], and
//! the [`audit`] trail.
//!
//! The safety-scanner contract —
//! [`SafetyScanner`](systemprompt_ai::SafetyScanner),
//! [`Finding`](systemprompt_ai::Finding), the built-in scanners, and
//! [`register_safety_scanner!`](systemprompt_ai::register_safety_scanner) —
//! lives in `systemprompt-ai`; [`registry::SafetyScannerRegistry`] resolves the
//! scanner names a policy selects against the built-ins plus any extension
//! registrations.

pub mod audit;
pub mod captures;
pub mod parse;
pub mod policy;
pub mod pricing;
pub mod protocol;
pub mod quota;
pub mod registry;
pub mod service;
pub mod stream_tap;

pub use audit::{GatewayAudit, GatewayRequestContext};
pub use captures::{CapturedToolUse, CapturedUsage};
pub use protocol::{
    CanonicalEvent, CanonicalRequest, CanonicalResponse, InboundAdapter, OutboundAdapter,
    OutboundAdapterRegistration, OutboundCtx, OutboundOutcome,
};
pub use registry::{GatewayUpstreamRegistry, SafetyScannerRegistry};
pub use service::{DispatchInputs, GatewayService, REQUEST_ID_HEADER};
