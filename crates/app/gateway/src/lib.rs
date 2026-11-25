//! `systemprompt-gateway` — the AI gateway: a protocol-translating proxy in
//! front of upstream LLM providers.
//!
//! Inbound requests in one wire protocol (Anthropic Messages, `OpenAI` Chat
//! Completions and Responses) are parsed into a canonical form, dispatched to
//! an upstream provider via the [`protocol`] adapters, and rendered back in the
//! caller's protocol. [`GatewayService`] orchestrates the flow; supporting
//! modules cover gateway [`policies`] (the declarative spec, its
//! `gateway_policies` config plane, resolution, and the safety-scanner,
//! route-selector and
//! system-prompt-override extension contracts), [`quota`] enforcement, usage
//! [`captures`], [`pricing`], the upstream and safety-scanner [`registry`]
//! with the [`contract`] slots members contribute to,
//! remote [`image_fetch`], the streaming [`stream_tap`], and the [`audit`]
//! trail with its encrypted settlement journal.
//!
//! Core services call models through the in-process [`client`], which enters
//! the same pipeline as a service principal.
//!
//! The crate owns no handlers: the [`surface`] contract is what the
//! `gateway-http` member implements and the API mounts, and the handlers call
//! into [`GatewayService`] with the repositories bundled in
//! [`GatewayRepositories`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod artifact_scanner;
pub mod at_rest;
pub mod audit;
pub mod captures;
pub mod client;
pub mod contract;
pub mod error;
pub mod extension;
pub mod image_fetch;
pub mod parse;
pub mod policies;
pub mod pricing;
pub mod protocol;
pub mod quota;
pub mod registry;
pub mod repository;
pub mod routing;
pub mod service;
pub mod signature_cache;
pub mod stream_tap;
pub mod surface;

pub use artifact_scanner::GatewayArtifactScanner;
pub use at_rest::{GatewayAtRest, GatewayAtRestError};
pub use audit::metrics::{
    GATEWAY_OVERHEAD_SECONDS, GATEWAY_UPSTREAM_DURATION_SECONDS, OVERHEAD_BUCKETS, UPSTREAM_BUCKETS,
};
pub use audit::phases::{
    GATEWAY_PHASE_SECONDS, GATEWAY_STREAMS_IN_FLIGHT, GatewayPhase, PHASE_BUCKETS, StreamInFlight,
};
pub use audit::{GatewayAudit, GatewayRequestContext};
pub use captures::CapturedToolUse;
pub use error::{GatewayAuditError, GatewayAuditResult};
pub use policies::{
    CATEGORY_SCANNER_FAILURE, CalendarPeriod, Finding, FindingSpan, GATEWAY_POLICIES_FILE,
    GatewayPoliciesPlane, GatewayPolicyConfig, GatewayPolicyEntry, GatewayPolicyError,
    GatewayPolicySpec, HeuristicConfig, HeuristicScanner, NullScanner, OverrideAction,
    OverrideContext, OverrideContextBuilder, OverrideError, OverridePolicy, OverrideResolution,
    OverrideSource, PHASE_REQUEST, PHASE_REQUEST_HISTORY, PHASE_RESPONSE, PartReplacement,
    PolicyResolver, PolicyUnavailable, QuotaWindow, RouteSelector, RouteSelectorChain,
    RouteSelectorError, SafetyConfig, SafetyHistoryMode, SafetyScanner, ScanError, ScannerFactory,
    ScannerFailMode, ScannerSettings, Severity, SystemPromptOverride, WindowSpan,
};
pub use protocol::{
    CanonicalEvent, CanonicalRequest, CanonicalResponse, InboundAdapter, OutboundAdapter,
    OutboundCtx, OutboundOutcome,
};
pub use registry::{
    GatewayEngines, GatewayEnginesError, GatewayUpstreamRegistry, SafetyScannerRegistry,
};
pub use repository::{GatewayHandles, GatewayRepositories};
pub use routing::{RouteResolution, RouteSource, resolve_route};
pub use service::{DispatchInputs, GatewayService, REQUEST_ID_HEADER};
pub use surface::{
    DynGatewaySurface, GatewayHostHandles, GatewayLogIdentity, GatewayRouteState, GatewaySurface,
    GatewaySurfaceError, GatewaySurfaceRoutes,
};
