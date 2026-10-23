//! The model catalog: five objects, each with one job.
//!
//! | file (`services/ai/`) | object | job |
//! |---|---|---|
//! | `credentials.yaml` | [`CredentialEntry`] | who authenticates and who pays |
//! | `providers.yaml` | [`ProviderConfig`] | the vendor account: headers, user agent, governance |
//! | (under a provider) | [`EndpointConfig`] | one wire plus one URL, betas, region, price multiplier |
//! | `models.yaml` | [`ModelEntry`] | what a model is, stated once |
//! | `deployments.yaml` | [`DeploymentTarget`] | which model is served where; order is failover |
//!
//! `gateway.yaml` adds the one `default_model` and the policy routes. The
//! files are read into a [`CatalogDocument`] and resolved into a
//! [`ResolvedCatalog`], which validates every reference and answers
//! `model → deployment → endpoint → provider → credential` for a request.
//! [`DefaultCatalog`] is core's embedded default in the same shape, and
//! [`vertex`] holds the Vertex rate card discovery prices models from.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod credentials;
mod default;
mod deployments;
mod document;
mod endpoint;
mod error;
mod headers;
mod models;
mod providers;
mod resolved;
pub mod vertex;

pub use credentials::{CredentialEntry, CredentialKind, CredentialRegistry, PlatformSecretKind};
pub use default::DefaultCatalog;
pub use deployments::{DeploymentDetail, DeploymentRegistry, DeploymentTarget, PricingOverride};
pub use document::{CATALOG_FILES, CatalogDocument, detect_legacy_shape};
pub use endpoint::{BetaSpec, EndpointConfig, EndpointHosting, EndpointRef, EndpointRefError};
pub use error::{CatalogDocumentError, CatalogError, CatalogResult, MIGRATE_HINT, ResolveError};
pub use models::{
    Capability, ModelEntry, ModelRegistry, apply_capability_overrides, capability_flags,
};
pub use providers::{CatalogContext, ProviderConfig, ProviderRegistry};
pub use resolved::{
    CatalogInput, ModelRequest, ResolvedCatalog, ResolvedCredential, ResolvedEndpoint, ResolvedHop,
    ResolvedModel, ResolvedProvider, ResolvedTarget,
};
