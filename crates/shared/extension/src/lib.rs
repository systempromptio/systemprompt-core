//! The composition contract for systemprompt.io members.
//!
//! A member implements [`Extension`]: [`Extension::metadata`] and one
//! [`Extension::contribute`] that declares, into the member's own sink,
//! everything it brings — schemas, migrations, state providers, jobs,
//! retention, factories for the kernel-consumed services, capability
//! [`Slot`] entries, and with the `router` and `cli` features its route sets
//! and CLI commands. Every declaration only stores. The `systemprompt`
//! facade composes members into a [`Composition`](https://docs.rs/systemprompt)
//! and admits them against the profile; the kernel reads the resulting
//! [`Admitted`] handoff and re-checks every privileged entry against the
//! constants in [`reserved`].
//!
//! # Authoring a member
//!
//! ```ignore
//! use systemprompt_extension::prelude::*;
//!
//! struct MyExtension;
//!
//! impl Extension for MyExtension {
//!     fn metadata(&self) -> ExtensionMetadata {
//!         ExtensionMetadata { id: "my-ext", name: "My Extension", version: "0.1.0" }
//!     }
//!
//!     fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
//!         c.migrations(extension_migrations!()).job(MyJob);
//!         Ok(())
//!     }
//! }
//! ```
//!
//! # Module map
//!
//! - [`contribute`] — [`Contributions`], the member's sink, [`Resolved`] and
//!   [`FromState`], deferred [`ProvideFn`]s and [`Factory`]s, [`Slot`]s and
//!   kernel handles.
//! - [`admitted`] — the [`Admitted`] and [`DeclaredCommands`] handoffs.
//! - [`reserved`] — the kernel's reserved API prefixes, built-in gateway
//!   prefixes, reserved CLI paths and privileged route shapes.
//! - [`router`] — route bases, specs and shapes; with the `router` feature,
//!   [`router::RouteSet`] and the HTTP helpers capability handlers use.
//! - [`cli`] — [`cli::KernelGrant`]; with the `cli` feature, command
//!   declarations and the [`cli::CommandHost`] surface.
//! - [`protection`], [`authz`] — the protection a mounted group carries.
//! - [`metadata`], [`migration`], [`seed`], [`purge`], [`reassignment`],
//!   [`retention`], [`projection`] — the data a member declares.
//! - [`build`] — build-script helper that generates the migration list from
//!   `schema/migrations/*.sql`, read with [`extension_migrations!`].
//! - [`brain`], [`notifications`] — the seam traits those capabilities' slots
//!   collect.
//! - [`error`] — typed error enums ([`LoaderError`], [`ExtensionStateError`],
//!   [`ExtensionConfigError`], and the [`MigrationFault`] and [`SchemaFault`]
//!   causes).
//!
//! # Feature flags
//!
//! - `router` — axum route sets, the capability HTTP error, content negotiation
//!   and server-sent events.
//! - `cli` — clap command declarations and the command host.
//! - `slack`, `teams` — the Slack and Teams privileged shapes.
//! - `sqlx` — the brain and notification sinks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod admitted;
pub mod admitted_assets;
pub mod admitted_configs;
pub mod admitted_doctor;
pub mod admitted_migrations;
pub mod admitted_purge;
pub mod admitted_reassignment;
pub mod admitted_schedule;
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
pub mod cli;
pub mod compat;
pub mod contribute;
pub mod cost;
pub mod error;
pub mod frame_options;
pub mod gateway_guard;
pub mod health;
#[cfg(feature = "mcp")]
pub mod mcp;
pub mod metadata;
pub mod migration;
pub mod notifications;
pub mod projection;
pub mod protection;
pub mod purge;
pub mod reassignment;
pub mod reserved;
pub mod retention;
pub mod router;
pub mod seed;
pub mod service_process;
pub mod state;
mod traits;

pub use admitted::{Admitted, BuiltState, DeclaredCommands};
pub use admitted_assets::AdmittedAssets;
pub use admitted_configs::{
    AdmittedConfigs, ExtensionConfigFileError, ExtensionConfigOutcome, validate_config,
};
pub use admitted_doctor::{AdmittedDoctorChecks, OwnedDoctorCheck};
pub use admitted_migrations::AdmittedMigrations;
pub use admitted_purge::{
    AdmittedPurge, OwnedPurge, PurgeAdmissionError, PurgeClaim, created_table_names,
    purge_declarations,
};
pub use admitted_reassignment::{
    AdmittedOwnerReassignments, OwnedReassignment, ReassignmentAdmissionError, ReassignmentClaim,
};
pub use admitted_schedule::{AdmittedSchedule, DuplicateJobOwner};
pub use asset::{AssetDefinition, AssetDefinitionBuilder, AssetPaths, AssetType};
pub use authz::{AuthzPolicy, HandlerVerifier};
pub use brain::{
    AnalysisWindow, BrainAnalysis, BrainAnalysisError, BrainExtractError, BrainModelClient,
    BrainModelError, BrainSinkError, BrainValidateError, EventFilter, FactExtractor,
    FactRecallSource, FactRetriever, FactValidator, FilterVerdict, ImportanceModel, InjectedFact,
    InjectionPolicy, MemoryRecall, OutcomeSignal, OutcomeSignalError, RecallBlock,
    RecallConversation, RecallError, RecallObservation, RecallOutcome, RecallPrincipal,
    RecallReceipt, RecallRequest, ReuseHint, SkipReason, SourceAccess, SourceAccessError,
    SourceContent, SourceContentError, SourceVerdict, ValidationVerdict, WithheldSummary,
};
#[cfg(feature = "sqlx")]
pub use brain::{BrainSink, DynBrainSink};
pub use contribute::{
    AudienceEntry, AudienceResolvers, ContributionError, Contributions, ExtensionConfig, Factory,
    FactoryError, FactoryKind, FromState, Handed, HandleId, KernelHandle, LateCell, MemberResolved,
    MemberScope, MemberSecrets, ProvideError, ProvideFn, ProvidePhase, Resolved,
    SecretDeclarations, Slot, SlotEntry, SlotError, SlotOrder, StandingTag, build_factories,
    handles,
};
pub use cost::{CostDirective, CostDirectiveError, TriggerPolicy};
pub use error::{
    ExtensionConfigError, ExtensionStateError, LoaderError, MigrationFault, SchemaFault,
};
pub use frame_options::FrameOptionsOverride;
#[cfg(feature = "router")]
pub use frame_options::stamp_frame_options;
pub use gateway_guard::{
    GatewayDenyKind, GatewayDenyReason, GatewayGuardRequest, GatewayGuards, GatewayRequestGuard,
};
pub use health::{
    BoxedReadinessSource, DiagnosticDetail, DiagnosticFailure, DiagnosticProbe, MetricDescriptor,
    MetricKind, ReadinessCheck, ReadinessFailure,
};
pub use metadata::{ExtensionMetadata, ExtensionRole, SchemaDefinition};
pub use migration::Migration;
pub use notifications::{
    ChannelError, Membership, NotificationAudienceError, NotificationAudienceResolver,
    NotificationChannel, NotificationRouter, NotificationRouterError, NotificationSinkError,
    PublishPolicy, PublishPolicyError, PublishVerdict, ResourceVisibility, Visibility,
    VisibilityError,
};
#[cfg(feature = "sqlx")]
pub use notifications::{DynNotificationSink, NotificationSink};
pub use projection::{AdmittedProjections, DrainTarget, ProjectionDrain};
pub use protection::{
    ContextKind, ProtectionLayer, RateBudget, RateLimit, RouteGroup, RouteProtection,
    SiteAuthConfig,
};
pub use purge::{OrphanSweep, PurgeDeclaration, PurgeFilter, PurgePredicate, UserPurgeTable};
pub use reassignment::{OwnerReassignment, ReassignAction};
pub use retention::{
    EffectiveCutoff, RetentionDecl, RetentionHold, RetentionHoldError, RetentionHolds,
    RetentionTarget,
};
pub use seed::Seed;
pub use state::{StateHandle, StateRequirement};
pub use systemprompt_models::{AlreadyBound, LateBinder, LateBound, LateRead, late_pair};
#[cfg(feature = "sqlx")]
pub use systemprompt_provider_contracts::DoctorProbeSource;
pub use systemprompt_provider_contracts::MissingDependency;
pub use traits::Extension;

pub use capabilities::HasRouteClassifier;

pub mod prelude {
    pub use crate::asset::{AssetDefinition, AssetDefinitionBuilder, AssetPaths, AssetType};
    pub use crate::contribute::{
        ContributionError, Contributions, ExtensionConfig, Factory, FactoryError, FromState,
        Handed, MemberResolved, MemberScope, ProvideError, Resolved, Slot, SlotEntry, SlotError,
        SlotOrder, StandingTag,
    };
    pub use crate::error::{ExtensionConfigError, ExtensionStateError, LoaderError};
    pub use crate::gateway_guard::{
        GatewayDenyKind, GatewayDenyReason, GatewayGuardRequest, GatewayGuards, GatewayRequestGuard,
    };
    pub use crate::reserved::ConferralRow;
    pub use crate::retention::{RetentionDecl, RetentionHold, RetentionHoldError, RetentionTarget};
    pub use crate::seed::Seed;
    #[cfg(feature = "router")]
    pub use crate::stamp_frame_options;
    pub use crate::state::{StateHandle, StateRequirement};
    pub use crate::{
        AuthzPolicy, ContextKind, Extension, ExtensionMetadata, ExtensionRole, Migration,
        MissingDependency, OrphanSweep, OwnerReassignment, PurgePredicate, RateBudget, RateLimit,
        ReassignAction, RouteProtection, SchemaDefinition, SiteAuthConfig, UserPurgeTable,
        extension_migrations,
    };
    pub use systemprompt_models::{LateBound, LateRead};

    pub use systemprompt_provider_contracts::governance::RuleType;
    pub use systemprompt_provider_contracts::{
        ApplyMode, ApplyOutcome, ApplyRequest, ChangeActor, ChangeRequest, ChangeSource,
        ComponentContext, ComponentRenderer, ConfigPlane, ConfigPlaneError, ConfigPlaneResult,
        ConfigPlaneSet, ContentDataContext, ContentDataProvider, DeclaredEntities, DeclaredEntity,
        DeclaredState, Dependencies, DependencyProvider, DriftKind, DriftRow, DuplicateDependency,
        EffectiveAuthority, EntityScope, FrameOptions, FrontmatterContext, FrontmatterProcessor,
        PageContext, PageDataProvider, PagePrepareContext, PagePrerenderer, PageRenderSpec,
        PlaceholderMapping, PlaneContext, PlaneEffect, PlaneSnapshot, PlaneTx, PlaneWriter,
        RenderedComponent, ResolvedEntities, ResolvedEntity, RssFeedContext, RssFeedItem,
        RssFeedMetadata, RssFeedProvider, RssFeedSpec, SitemapContext, SitemapProvider,
        SitemapSourceSpec, SitemapUrlEntry, TemplateDataExtender, TemplateDefinition,
        TemplateProvider, TemplateSource,
    };
}
