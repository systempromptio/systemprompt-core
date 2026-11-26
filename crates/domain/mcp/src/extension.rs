//! [`McpExtension`]: contributes the MCP tables, migrations, jobs and
//! retention purge, and provides the tool-execution lookup and the MCP server
//! registry (`Arc<dyn McpRegistryProvider>`, which sign-in reads for MCP
//! resource scopes) and the read-only `Arc<McpServiceObserver>` (server
//! health, recorded process ids and the running set; no spawner, no
//! secrets), the tool-usage reports over its report views
//! (`DynToolUsageReport`, open: the analytics `tools` commands read it), the
//! MCP session store (`DynMcpSessionStore`, **sealed**: a restore lookup takes
//! the server and user to match, so only the toolset host reads it, with the
//! principal it verified), the artifact ingest (`Arc<ArtifactIngest>`, open: it
//! scans with the installation's secret scanner cell and records tool results)
//! and the same ingest as the shared `DynToolResultIngest` contract the gateway
//! reads (open, as the ingest is), the built UI renderers as the shared
//! `DynArtifactUiRenderer` the agent artifact routes read (open, after
//! governance), and, once governance has loaded, the per-server tool executions
//! (`DynToolExecutions`, open: an execution re-verifies the caller's hop token
//! and records usage only for the context that token vouches for, and reads no
//! secret) and the tool provider the ai member's inference path reads
//! (`Arc<dyn ToolProvider>`, **sealed**: it calls servers with the
//! connector-token cache and the secret lookup; provided before governance so
//! the ai member's after-governance inference provide sees it whatever the
//! composition order). It opens the [`UiSlot`] and the extension contract's
//! [`AccessRecorderSlot`], so both built values are present whenever this
//! member is admitted.
//!
//! It owns the schema objects that cannot exist without its tables: the
//! `tool_usage_rollup_hourly` rollup (its triggers sit on
//! `mcp_tool_executions`), the `tool_call_ledger` view and the analytics
//! report views over its tables. The ledger joins the ai member's tool-call
//! intents and the rollup reads the agent of a task from the ai member's
//! `ai_task_agents`, so the member requires the ai member's tool-call intent
//! claims (`DynToolCallIntentClaims`), and a composition without `ai` refuses
//! it by name.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_config::paths::AppPaths;
use systemprompt_database::{DbPool, ServiceRepository};
use systemprompt_extension::artifact_ui::DynArtifactUiRenderer;
use systemprompt_extension::mcp::artifacts::DynToolResultIngest;
use systemprompt_extension::mcp::{AccessRecorderSlot, DynMcpSessionStore, DynToolExecutions};
use systemprompt_extension::prelude::*;
use systemprompt_extension::service_process::ServiceProcessSlot;
use systemprompt_governance::policy::secrets::SecretScannerCell;
use systemprompt_identifiers::InstanceId;
use systemprompt_manifest::Config;
use systemprompt_manifest::services::{ServicesView, SystemAdmin};
use systemprompt_security::credential::TokenCache;
use systemprompt_security::keys::VerificationKeyring;
use systemprompt_traits::usage_reports::mcp::DynToolUsageReport;
use systemprompt_traits::{
    DynServiceProcessSet, DynToolCallIntentClaims, DynToolExecutionLookup, McpRegistryProvider,
    SecretLookup, ToolProvider,
};

use crate::contract::UiSlot;
use crate::repository::{McpSessionRepository, ToolUsageReportRepository, ToolUsageRepository};
use crate::services::McpServiceObserver;
use crate::services::artifact_ingest::ArtifactIngest;
use crate::services::process_set::McpProcessSet;
use crate::services::registry::RegistryService;
use crate::services::tool_provider::McpToolProvider;
use crate::services::ui_renderer::UiRendererRegistry;
use crate::tool::{CallerVerification, McpToolExecutions};
use systemprompt_models::audit_spans::DynAuditSpanSink;

#[derive(Debug, Clone, Copy, Default)]
pub struct McpExtension;

impl Extension for McpExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "mcp",
            name: "MCP",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        c.domain_config(systemprompt_manifest::validators::McpConfigValidator::boxed);
        crate::schemas::contribute_schemas(c);
        c.migrations(extension_migrations!())
            .requires::<DynToolCallIntentClaims>()
            .provide(
                |r: &Resolved<'_>| -> Result<DynToolExecutionLookup, ProvideError> {
                    let db = r.get::<DbPool>()?;
                    Ok(Arc::new(ToolUsageRepository::new(&db)))
                },
            )
            .provide(
                |r: &Resolved<'_>| -> Result<DynToolUsageReport, ProvideError> {
                    Ok(Arc::new(ToolUsageReportRepository::new(
                        &r.get::<DbPool>()?,
                    )))
                },
            )
            .provide(
                |r: &Resolved<'_>| -> Result<Arc<dyn McpRegistryProvider>, ProvideError> {
                    Ok(Arc::new(RegistryService::new(
                        r.get::<SystemAdmin>()?.id().clone(),
                        r.get::<ServicesView>()?,
                        r.get::<Arc<Config>>()?,
                    )))
                },
            );
        c.provide(
            |r: &Resolved<'_>| -> Result<Arc<McpServiceObserver>, ProvideError> {
                let db = r.get::<DbPool>()?;
                let registry = RegistryService::new(
                    r.get::<SystemAdmin>()?.id().clone(),
                    r.get::<ServicesView>()?,
                    r.get::<Arc<Config>>()?,
                );
                Ok(Arc::new(McpServiceObserver::new(
                    ServiceRepository::new(&db, r.get::<InstanceId>()?),
                    r.get::<Arc<AppPaths>>()?,
                    registry,
                )))
            },
        );
        contribute_stores(c);
        contribute_tool_provider(c);
        contribute_tool_executions(c);
        c.slot::<ServiceProcessSlot>().process_set(
            |r: &Resolved<'_>| -> Result<DynServiceProcessSet, FactoryError> {
                let services = ServiceRepository::new(&r.get::<DbPool>()?, r.get::<InstanceId>()?);
                Ok(Arc::new(McpProcessSet::from_resolved(r, services)?))
            },
        );
        crate::jobs::contribute(c);
        c.slot::<UiSlot>();
        c.slot::<AccessRecorderSlot>();
        crate::user_data::contribute(c);
        Ok(())
    }
}

fn contribute_tool_executions(c: &mut Contributions<'_>) {
    c.provide_after_governance(
        |r: &Resolved<'_>| -> Result<DynArtifactUiRenderer, ProvideError> {
            let renderers: DynArtifactUiRenderer = r.get::<Arc<UiRendererRegistry>>()?;
            Ok(renderers)
        },
    );
    c.provide_after_governance(
        |r: &Resolved<'_>| -> Result<DynToolExecutions, ProvideError> {
            let db = r.get::<DbPool>()?;
            let intents = r.get::<DynToolCallIntentClaims>()?;
            Ok(Arc::new(McpToolExecutions::new(
                Arc::new(
                    ToolUsageRepository::new(&db)
                        .with_audit_spans(r.optional::<DynAuditSpanSink>()),
                ),
                Some(intents),
                r.get::<Arc<ArtifactIngest>>()?,
                r.get::<Arc<UiRendererRegistry>>()?,
                CallerVerification::new(
                    r.get::<VerificationKeyring>()?,
                    &r.get::<Arc<Config>>()?.jwt_issuer,
                ),
            )))
        },
    );
}

fn contribute_tool_provider(c: &mut Contributions<'_>) {
    c.provide_sealed(
        |r: &Resolved<'_>| -> Result<Arc<dyn ToolProvider>, ProvideError> {
            let services = r.get::<ServicesView>()?;
            let resilience = services.config().ai.mcp.resilience;
            let registry = RegistryService::new(
                r.get::<SystemAdmin>()?.id().clone(),
                services,
                r.get::<Arc<Config>>()?,
            );
            Ok(Arc::new(McpToolProvider::new(
                r.get::<DbPool>()?,
                registry,
                &resilience,
                r.get::<Arc<TokenCache>>()?,
                r.get::<Arc<dyn SecretLookup>>()?,
            )))
        },
    );
}

fn contribute_stores(c: &mut Contributions<'_>) {
    c.provide_sealed(
        |r: &Resolved<'_>| -> Result<DynMcpSessionStore, ProvideError> {
            Ok(Arc::new(McpSessionRepository::new(&r.get::<DbPool>()?)))
        },
    )
    .provide(
        |r: &Resolved<'_>| -> Result<Arc<ArtifactIngest>, ProvideError> {
            Ok(Arc::new(ArtifactIngest::new(
                crate::repository::ArtifactIngestRepositories::new(&r.get::<DbPool>()?)
                    .with_audit_spans(r.optional::<DynAuditSpanSink>()),
                r.get::<SecretScannerCell>()?,
            )))
        },
    )
    .provide(
        |r: &Resolved<'_>| -> Result<DynToolResultIngest, ProvideError> {
            let ingest: DynToolResultIngest = r.get::<Arc<ArtifactIngest>>()?;
            Ok(ingest)
        },
    );
}
