//! Gateway policies: the declarative spec, its bootstrap and its resolution.
//!
//! Gateway policies (per-call ceilings, quota windows, safety config) live in
//! `ai_gateway_policies`, read on every request. A committed
//! `services/gateway/policies.yaml` reaches the database only through the
//! `gateway_policies` config plane ([`GatewayPoliciesPlane`]), applied by
//! `config apply`; boot never writes it. Model exposure is owned by the
//! profile catalog, not by this spec. [`PolicyResolver`] merges the stored rows
//! into the effective [`GatewayPolicySpec`] a request is governed by.
//!
//! The extension contracts a policy selects by name live here too: safety
//! scanners ([`register_safety_scanner!`](crate::register_safety_scanner)),
//! route selectors and system-prompt overrides.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod config;
mod error;
pub mod overrides;
mod plane;
mod resolver;
pub mod route_selector;
pub mod safety;
mod spec;

pub use config::{GatewayPolicyConfig, GatewayPolicyEntry};
pub use error::GatewayPolicyError;
pub use overrides::{
    OverrideAction, OverrideContext, OverrideContextBuilder, OverrideError, OverridePolicy,
    OverrideResolution, OverrideSource, SystemPromptOverride, SystemPromptOverrideRegistration,
};
pub use plane::{GATEWAY_POLICIES_FILE, GATEWAY_POLICIES_PLANE, GatewayPoliciesPlane};
pub use resolver::{
    MalformedPolicy, MalformedSpec, PolicyResolver, PolicyUnavailable, merge_policy_rows,
};
pub use route_selector::{
    RouteSelector, RouteSelectorChain, RouteSelectorError, RouteSelectorRegistration,
};
pub use safety::{
    CATEGORY_SCANNER_FAILURE, Finding, FindingSpan, HeuristicScanner, NullScanner, PHASE_REQUEST,
    PHASE_REQUEST_HISTORY, PHASE_RESPONSE, PartReplacement, SafetyScanner,
    SafetyScannerRegistration, ScanError, ScannerFactory, Severity,
};
pub use spec::{
    CalendarPeriod, DEFAULT_SCANNER_TIMEOUT_MS, GatewayPolicySpec, HeuristicConfig, QuotaWindow,
    RateWindow, SafetyConfig, SafetyHistoryMode, ScannerFailMode, ScannerSettings, WindowSpan,
};
