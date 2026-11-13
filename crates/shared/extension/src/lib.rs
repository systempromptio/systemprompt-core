//! Compile-time extension framework for systemprompt.io.
//!
//! Built on the [`inventory`] crate, this crate lets extension authors
//! declare schemas, API routes, scheduled jobs, and provider
//! implementations using the [`Extension`] trait and the
//! [`register_extension!`] macro. The runtime collects every registration
//! at startup, validates dependencies, and merges the resulting wiring
//! into the host application.
//!
//! # Authoring an extension
//!
//! ```ignore
//! use systemprompt_extension::prelude::*;
//!
//! #[derive(Default)]
//! struct MyExtension;
//!
//! impl Extension for MyExtension {
//!     fn metadata(&self) -> ExtensionMetadata {
//!         ExtensionMetadata { id: "my-ext", name: "My Extension", version: "0.1.0" }
//!     }
//! }
//!
//! register_extension!(MyExtension);
//! ```
//!
//! # Module map
//!
//! - [`metadata`], [`router`], [`migration`] — value types describing the
//!   extension surface; [`authz`] — the route authorization tier a router
//!   declaration names.
//! - The `Extension` trait and the [`register_extension!`] macro form the core
//!   extensibility contract.
//! - [`build`] — build-script helper that generates `Extension::migrations()`
//!   from `schema/migrations/*.sql` files, paired with the
//!   [`extension_migrations!`] macro.
//! - [`brain`] — the organisational-memory seams: outbox sink, capture
//!   pipeline, recall and measurement, registered with `register_brain_*!`.
//! - [`purge`] — the user-keyed tables a deleted user's rows leave with them,
//!   declared per crate with [`user_purge_tables!`], and the shared rows swept
//!   once nothing references them, declared with [`orphan_sweeps!`].
//! - [`retention`] — the tables the retention job prunes, and the holds an
//!   extension registers with [`register_retention_hold!`] to keep rows it has
//!   not yet released.
//! - [`registry`] — runtime store of registered extensions, built from
//!   `inventory` submissions at startup.
//! - [`state`] — typed, extension-scoped state: [`Extension::provide`]
//!   contributes values at boot in dependency order, [`Extension::requires`]
//!   declares what must be present, and the immutable state — one
//!   [`Dependencies`](systemprompt_provider_contracts::Dependencies) map seeded
//!   with the kernel's handles — is read by every factory through
//!   [`ExtensionContext::extension_state`]; an authz hook factory, which runs
//!   before the state exists, reads it through a [`StateHandle`].
//! - [`capabilities`] — `Has*` capability traits the host context implements to
//!   expose its subsystems to extensions.
//! - [`error`] — typed error enums ([`LoaderError`], [`ExtensionStateError`],
//!   [`ExtensionConfigError`], and the [`MigrationFault`] and [`SchemaFault`]
//!   causes) raised by the registry and the schema installer.
//!
//! # Feature flags
//!
//! This crate has no Cargo features; everything compiles into every build.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[doc(hidden)]
pub use ::inventory as __inventory;

mod asset;
pub mod authz;
pub mod brain;
#[expect(
    clippy::panic,
    clippy::print_stdout,
    reason = "build-script support code: a panic is the correct way to abort a build, and \
              println! is how a build script emits cargo directives"
)]
pub mod build;
pub mod capabilities;
pub mod compat;
pub mod context;
pub mod cost;
pub mod error;
pub mod frame_options;
pub mod gateway_guard;
pub mod health;
pub mod metadata;
pub mod migration;
pub mod notifications;
pub mod purge;
pub mod registry;
pub mod retention;
pub mod router;
mod seam;
pub mod seed;
pub mod state;
mod traits;

pub use asset::{AssetDefinition, AssetDefinitionBuilder, AssetPaths, AssetType};
pub use authz::AuthzPolicy;
pub use brain::{
    AnalysisWindow, BrainAnalysis, BrainAnalysisError, BrainAnalysisRegistration,
    BrainExtractError, BrainExtractorRegistration, BrainFilterRegistration,
    BrainImportanceRegistration, BrainInjectionPolicyRegistration, BrainModelClient,
    BrainModelError, BrainOutcomeSignalRegistration, BrainRetrieverRegistration, BrainSinkError,
    BrainSourceContentRegistration, BrainValidateError, BrainValidatorRegistration, EventFilter,
    FactExtractor, FactRecallSource, FactRetriever, FactValidator, FilterVerdict, ImportanceModel,
    InjectedFact, InjectionPolicy, MemoryRecall, OutcomeSignal, OutcomeSignalError, RecallBlock,
    RecallConversation, RecallError, RecallObservation, RecallOutcome, RecallPrincipal,
    RecallReceipt, RecallRequest, ReuseHint, SkipReason, SourceAccess, SourceAccessError,
    SourceContent, SourceContentError, SourceVerdict, ValidationVerdict, WithheldSummary,
    discover_brain_analyses, discover_brain_extractors, discover_brain_filters,
    discover_brain_importance_models, discover_brain_injection_policies,
    discover_brain_outcome_signals, discover_brain_retrievers, discover_brain_source_contents,
    discover_brain_validators,
};
#[cfg(feature = "sqlx")]
pub use brain::{BrainSink, DynBrainSink};
pub use context::{DynExtensionContext, ExtensionContext};
pub use cost::{CostDirective, CostDirectiveError, TriggerPolicy};
pub use error::{
    ExtensionConfigError, ExtensionStateError, LoaderError, MigrationFault, SchemaFault,
};
pub use frame_options::{FrameOptionsOverride, stamp_frame_options};
pub use gateway_guard::{
    GatewayDenyKind, GatewayDenyReason, GatewayGuardRequest, GatewayGuards, GatewayRequestGuard,
    GatewayRequestGuardRegistration,
};
pub use health::{
    BoxedReadinessSource, MetricDescriptor, MetricKind, ReadinessCheck, ReadinessFailure,
};
pub use metadata::{ExtensionMetadata, ExtensionRole, SchemaDefinition};
pub use migration::Migration;
pub use notifications::{
    ChannelError, Membership, NotificationAudienceError, NotificationAudienceRegistration,
    NotificationAudienceResolver, NotificationChannel, NotificationChannelRegistration,
    NotificationRouter, NotificationRouterError, NotificationRouterRegistration,
    NotificationSinkError, PublishPolicy, PublishPolicyError, PublishPolicyRegistration,
    PublishVerdict, ResourceVisibility, ResourceVisibilityRegistration, Visibility,
    VisibilityError, discover_notification_audiences, discover_notification_channels,
    discover_notification_publish_policies, discover_notification_routers,
    discover_resource_visibilities,
};
#[cfg(feature = "sqlx")]
pub use notifications::{DynNotificationSink, NotificationSink};
pub use purge::{UserPurgeTable, registered_user_purge_tables};
pub use registry::{ExtensionRegistration, ExtensionRegistry, RESERVED_PATHS};
pub use retention::{
    EffectiveCutoff, RetentionHold, RetentionHoldError, RetentionHoldRegistration, RetentionHolds,
    RetentionTarget, discover_retention_holds,
};
pub use router::{
    ContextKind, EnablePredicate, ExtensionRouter, ExtensionRouterConfig, ProtectionLayer,
    RateBudget, RateLimit, RouteGroup, RouteProtection, SiteAuthConfig,
};
pub use seed::Seed;
pub use state::{StateBuilder, StateHandle, StateRequirement};
pub use traits::Extension;

pub use capabilities::{HasAnalytics, HasFingerprint, HasRouteClassifier, HasUserService};

pub mod prelude {
    pub use crate::asset::{AssetDefinition, AssetDefinitionBuilder, AssetPaths, AssetType};
    pub use crate::context::{DynExtensionContext, ExtensionContext};
    pub use crate::error::{ExtensionConfigError, ExtensionStateError, LoaderError};
    pub use crate::gateway_guard::{
        GatewayDenyKind, GatewayDenyReason, GatewayGuardRequest, GatewayGuards, GatewayRequestGuard,
    };
    pub use crate::registry::ExtensionRegistry;
    pub use crate::retention::{RetentionHold, RetentionHoldError, RetentionTarget};
    pub use crate::seed::Seed;
    pub use crate::state::{StateBuilder, StateHandle, StateRequirement};
    pub use crate::{
        AuthzPolicy, ContextKind, Extension, ExtensionMetadata, ExtensionRole, ExtensionRouter,
        Migration, RateBudget, RateLimit, RouteProtection, SchemaDefinition, SiteAuthConfig,
        extension_migrations, register_extension, register_gateway_guard, register_retention_hold,
        stamp_frame_options, user_purge_tables,
    };

    pub use systemprompt_provider_contracts::{
        ApplyMode, ApplyOutcome, ApplyRequest, ChangeActor, ChangeRequest, ChangeSource,
        ComponentContext, ComponentRenderer, ConfigPlane, ConfigPlaneError, ConfigPlaneResult,
        ConfigPlaneSet, ContentDataContext, ContentDataProvider, DeclaredEntities, DeclaredEntity,
        DeclaredState, Dependencies, DependencyProvider, DriftKind, DriftRow, DuplicateDependency,
        EffectiveAuthority, EntityScope, FrameOptions, FrontmatterContext, FrontmatterProcessor,
        MissingDependency, PageContext, PageDataProvider, PagePrepareContext, PagePrerenderer,
        PageRenderSpec, PlaceholderMapping, PlaneContext, PlaneEffect, PlaneSnapshot, PlaneTx,
        PlaneWriter, RenderedComponent, ResolvedEntities, ResolvedEntity, RssFeedContext,
        RssFeedItem, RssFeedMetadata, RssFeedProvider, RssFeedSpec, SitemapContext,
        SitemapProvider, SitemapSourceSpec, SitemapUrlEntry, TemplateDataExtender,
        TemplateDefinition, TemplateProvider, TemplateSource,
    };
}
