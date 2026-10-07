//! `systemprompt-gateway` — the AI gateway: a protocol-translating proxy in
//! front of upstream LLM providers.
//!
//! Inbound requests in one wire protocol (Anthropic Messages, `OpenAI` Chat
//! Completions and Responses) are parsed into a canonical form, dispatched to
//! an upstream provider via the [`protocol`] adapters, and rendered back in the
//! caller's protocol. [`GatewayService`] orchestrates the flow; supporting
//! modules cover gateway [`policies`] (the declarative spec, its YAML
//! bootstrap, resolution, and the safety-scanner, route-selector and
//! system-prompt-override extension contracts), [`quota`] enforcement, usage
//! [`captures`], [`pricing`], the upstream and safety-scanner [`registry`],
//! remote [`image_fetch`], the streaming [`stream_tap`], and the [`audit`]
//! trail with its encrypted settlement journal.
//!
//! The crate owns no HTTP surface: `systemprompt-api` mounts the gateway
//! routes and calls into [`GatewayService`] with the repositories bundled in
//! [`GatewayRepositories`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod artifact_scanner;
pub mod audit;
pub mod captures;
pub mod error;
pub mod image_fetch;
pub mod parse;
pub mod policies;
pub mod pricing;
pub mod protocol;
pub mod quota;
pub mod registry;
pub mod repository;
pub mod service;
pub mod signature_cache;
pub mod stream_tap;

pub use artifact_scanner::GatewayArtifactScanner;
pub use audit::{GatewayAudit, GatewayRequestContext};
pub use captures::CapturedToolUse;
pub use error::{GatewayAuditError, GatewayAuditResult};
pub use policies::{
    API_KEY_QUOTA_SUBJECT, CATEGORY_SCANNER_FAILURE, Finding, GATEWAY_POLICIES_FILE,
    GatewayPolicyConfig, GatewayPolicyEntry, GatewayPolicyError, GatewayPolicyIngestionService,
    GatewayPolicySpec, HeuristicConfig, HeuristicScanner,
    IngestOptions as GatewayPolicyIngestOptions, IngestReport as GatewayPolicyIngestReport,
    NullScanner, OverrideAction, OverrideContext, OverrideContextBuilder, OverrideEngine,
    OverrideError, OverrideResolution, OverrideSource, PHASE_REQUEST, PHASE_REQUEST_HISTORY,
    PHASE_RESPONSE, PolicyResolver, PolicyUnavailable, QuotaMode, QuotaWindow, RouteSelector,
    RouteSelectorEngine, RouteSelectorError, RouteSelectorRegistration, SafetyConfig,
    SafetyHistoryMode, SafetyMode, SafetyScanner, SafetyScannerRegistration, ScanError,
    ScannerFactory, Severity, SystemPromptOverride, SystemPromptOverrideRegistration,
    USER_QUOTA_SUBJECT, load_from_yaml as load_gateway_policies_from_yaml,
};
pub use protocol::{
    CanonicalEvent, CanonicalRequest, CanonicalResponse, InboundAdapter, OutboundAdapter,
    OutboundAdapterRegistration, OutboundCtx, OutboundOutcome,
};
pub use registry::{GatewayUpstreamRegistry, SafetyScannerRegistry};
pub use repository::GatewayRepositories;
pub use service::{DispatchInputs, GatewayService, REQUEST_ID_HEADER};
