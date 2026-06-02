//! Declarative, version-controlled gateway-policy bootstrap.
//!
//! Gateway policies (per-call ceilings, quota windows, safety config) live in
//! `ai_gateway_policies`. This module gives them the same config-driven
//! bootstrap path that access-control rules already have: a committed
//! `services/gateway/policies.yaml` is ingested into the DB at every server
//! boot via [`load_from_yaml`]. Model exposure is owned by the profile
//! catalog, not by this spec.

mod config;
mod ingestion;
mod loader;
pub mod safety;
mod spec;

pub use config::{GatewayPolicyConfig, GatewayPolicyEntry};
pub use ingestion::{GatewayPolicyIngestionService, IngestOptions, IngestReport};
pub use loader::{GATEWAY_POLICIES_FILE, load_from_yaml};
pub use safety::{
    Finding, HeuristicScanner, NullScanner, SafetyScanner, SafetyScannerRegistration, Severity,
};
pub use spec::{GatewayPolicySpec, QuotaWindow, SafetyConfig};
