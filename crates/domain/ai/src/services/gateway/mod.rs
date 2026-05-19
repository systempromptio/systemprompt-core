//! Declarative, version-controlled gateway-policy bootstrap.
//!
//! Gateway policies (the inference-model allow-list and per-call governance
//! ceilings) live in `ai_gateway_policies`. This module gives them the same
//! config-driven bootstrap path that access-control rules already have: a
//! committed `services/ai/gateway-policies.yaml` is ingested into the DB at
//! every server boot via [`load_from_yaml`].

mod config;
mod ingestion;
mod loader;
mod spec;

pub use config::{GatewayPolicyConfig, GatewayPolicyEntry};
pub use ingestion::{GatewayPolicyIngestionService, IngestOptions, IngestReport};
pub use loader::{GATEWAY_POLICIES_FILE, load_from_yaml};
pub use spec::{GatewayPolicySpec, QuotaWindow, SafetyConfig};
