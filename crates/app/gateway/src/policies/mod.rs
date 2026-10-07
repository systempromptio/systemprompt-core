//! Gateway policies: the declarative spec, its bootstrap and its resolution.
//!
//! Gateway policies (per-call ceilings, quota windows, safety config) live in
//! `ai_gateway_policies`. This module gives them the same config-driven
//! bootstrap path that access-control rules already have: a committed
//! `services/gateway/policies.yaml` is ingested into the DB via
//! [`load_from_yaml`]. Model exposure is owned by the profile catalog, not by
//! this spec. [`PolicyResolver`] merges the stored rows into the effective
//! [`GatewayPolicySpec`] a request is governed by.
//!
//! The extension contracts a policy selects by name live here too: safety
//! scanners ([`register_safety_scanner!`](crate::register_safety_scanner)),
//! route selectors and system-prompt overrides.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod config;
mod error;
mod ingestion;
mod loader;
pub mod overrides;
mod resolver;
pub mod route_selector;
pub mod safety;
mod spec;

pub use config::{GatewayPolicyConfig, GatewayPolicyEntry};
pub use error::GatewayPolicyError;
pub use ingestion::{GatewayPolicyIngestionService, IngestOptions, IngestReport};
pub use loader::{GATEWAY_POLICIES_FILE, load_from_yaml};
pub use overrides::{
    OverrideAction, OverrideContext, OverrideContextBuilder, OverrideEngine, OverrideError,
    OverrideResolution, OverrideSource, SystemPromptOverride, SystemPromptOverrideRegistration,
};
pub use resolver::{MalformedPolicy, PolicyResolver, PolicyUnavailable, merge_policy_rows};
pub use route_selector::{
    RouteSelector, RouteSelectorEngine, RouteSelectorError, RouteSelectorRegistration,
};
pub use safety::{
    CATEGORY_SCANNER_FAILURE, Finding, HeuristicScanner, NullScanner, PHASE_REQUEST,
    PHASE_REQUEST_HISTORY, PHASE_RESPONSE, SafetyScanner, SafetyScannerRegistration, ScanError,
    ScannerFactory, Severity,
};
pub use spec::{
    API_KEY_QUOTA_SUBJECT, DEFAULT_SCANNER_TIMEOUT_MS, GatewayPolicySpec, HeuristicConfig,
    QuotaMode, QuotaWindow, SafetyConfig, SafetyHistoryMode, SafetyMode, ScannerFailMode,
    ScannerSettings, USER_QUOTA_SUBJECT,
};
