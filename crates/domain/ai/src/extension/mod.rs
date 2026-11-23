//! [`AiExtension`]: contributes the AI tables and migrations, the values it
//! provides, and the AI jobs, retention purges and rollup drains.
//!
//! Open: the request attribution lookup and the tool-call intent claims, both
//! database-only and keyed by the caller's own request or session.
//!
//! Sealed (kernel code and core provide fns read them; no member command,
//! job, runner, plane or route state does):
//! - `Arc<AiRepositories>`: it holds the at-rest cipher that decrypts users'
//!   personal provider keys, keyed from the sealed secret store;
//! - `DynSafetyFindingsReader` and `DynCatalogMigrationPort`, the two ports
//!   the `logs governance report` and `admin config migrate --data` kernel
//!   commands read;
//! - after governance, the process's one `DynAiProvider`: the [`AiService`]
//!   over the catalog, the sealed connector token cache and provider-key
//!   lookup, the sealed AI session provider the `users` member provides (read
//!   optionally: without it every call refuses), a sealed
//!   `Arc<dyn ToolProvider>` when one is provided (read optionally: without
//!   it the service lists and runs no tools) and the agent member's context
//!   materializer (optional). When the catalog serves no default model it is
//!   [`InferenceNotConfigured`], which answers every call with a typed
//!   configuration error.
//!
//! [`AiService`]: crate::AiService
//! [`InferenceNotConfigured`]: crate::services::core::InferenceNotConfigured
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_extension::prelude::*;
use systemprompt_models::attribution::DynRequestAttributionLookup;
use systemprompt_traits::{DynCatalogMigrationPort, DynSafetyFindingsReader};

use crate::repository::{AiRequestRepository, AiSafetyFindingRepository, CatalogMigrationRepository};

mod provides;

#[derive(Debug, Clone, Copy, Default)]
pub struct AiExtension;

impl Extension for AiExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "ai",
            name: "AI",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        for schema in request_schemas().into_iter().chain(gateway_schemas()) {
            c.schema(schema);
        }
        c.migrations(extension_migrations!())
            .provide(
                |r: &Resolved<'_>| -> Result<DynRequestAttributionLookup, ProvideError> {
                    let db = r.get::<DbPool>()?;
                    Ok(Arc::new(AiRequestRepository::new(&db)))
                },
            )
            .provide_sealed(provides::repositories)
            .provide_sealed(
                |r: &Resolved<'_>| -> Result<DynSafetyFindingsReader, ProvideError> {
                    let db = r.get::<DbPool>()?;
                    Ok(Arc::new(AiSafetyFindingRepository::new(&db)))
                },
            )
            .provide_sealed(
                |r: &Resolved<'_>| -> Result<DynCatalogMigrationPort, ProvideError> {
                    let db = r.get::<DbPool>()?;
                    Ok(Arc::new(CatalogMigrationRepository::new(&db)))
                },
            )
            .provide_sealed_after_governance(provides::inference);
        crate::jobs::contribute(c);
        c.user_purge(UserPurgeTable {
            table: "ai_requests",
            column: "user_id",
            predicate: None,
        });
        c.owner_reassignment(OwnerReassignment {
            table: "ai_requests",
            column: "user_id",
            action: ReassignAction::Update {
                touch: Some("updated_at"),
                keep_target_on: &[],
            },
        });
        c.owner_reassignment(OwnerReassignment {
            table: "ai_quota_buckets",
            column: "subject_id",
            action: ReassignAction::Delete {
                predicate: Some(PurgePredicate {
                    column: "subject_kind",
                    any_of: &["user"],
                }),
            },
        });
        c.owner_reassignment(OwnerReassignment {
            table: "ai_gateway_thought_signatures",
            column: "user_id",
            action: ReassignAction::Update {
                touch: None,
                keep_target_on: &["conversation_id", "tool_use_id"],
            },
        });
        Ok(())
    }
}

fn request_schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::sql_only(include_str!("../../schema/ai_request_message_count.sql")),
        SchemaDefinition::new("ai_requests", include_str!("../../schema/ai_requests.sql"))
            .with_required_columns(columns(&[
                "id",
                "provider",
                "model",
                "created_at",
                "client_kind",
                "wire_protocol",
                "client_attestation",
                "finish_reason",
                "served_provider",
                "api_key_id",
                "has_offered_tools",
            ])),
        SchemaDefinition::new(
            "ai_request_attributions",
            include_str!("../../schema/ai_request_attributions.sql"),
        )
        .with_required_columns(columns(&["request_id", "dimension", "value", "source"])),
        SchemaDefinition::new(
            "ai_request_client_evidence",
            include_str!("../../schema/ai_request_client_evidence.sql"),
        )
        .with_required_columns(columns(&["ai_request_id", "kind_source"])),
        SchemaDefinition::new(
            "ai_request_messages",
            include_str!("../../schema/ai_request_messages.sql"),
        )
        .with_required_columns(columns(&["id", "request_id", "role"])),
        SchemaDefinition::new(
            "ai_request_tool_calls",
            include_str!("../../schema/ai_request_tool_calls.sql"),
        )
        .with_required_columns(columns(&["id", "request_id", "tool_name"])),
        SchemaDefinition::new(
            "ai_tool_catalogs",
            include_str!("../../schema/ai_tool_catalogs.sql"),
        )
        .with_required_columns(columns(&["sha256", "tools"])),
        SchemaDefinition::new(
            "ai_request_payloads",
            include_str!("../../schema/ai_request_payloads.sql"),
        )
        .with_required_columns(columns(&["ai_request_id", "offered_tools_sha256"])),
        SchemaDefinition::sql_only(include_str!("../../schema/ai_request_tool_flag.sql")),
    ]
}

fn gateway_schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new(
            "ai_safety_findings",
            include_str!("../../schema/ai_safety_findings.sql"),
        )
        .with_required_columns(columns(&["id", "ai_request_id", "severity"])),
        SchemaDefinition::new(
            "ai_quota_buckets",
            include_str!("../../schema/ai_quota_buckets.sql"),
        )
        .with_required_columns(columns(&[
            "id",
            "subject_kind",
            "subject_id",
            "window_start",
        ])),
        SchemaDefinition::new(
            "ai_gateway_policies",
            include_str!("../../schema/ai_gateway_policies.sql"),
        )
        .with_required_columns(columns(&["id", "name", "spec"])),
        SchemaDefinition::sql_only(include_str!(
            "../../schema/ai_gateway_policies_cache_notify.sql"
        )),
        SchemaDefinition::sql_only(include_str!("../../schema/ai_gateway_policies_provenance.sql")),
        SchemaDefinition::new(
            "ai_gateway_thought_signatures",
            include_str!("../../schema/ai_gateway_thought_signatures.sql"),
        )
        .with_required_columns(columns(&[
            "conversation_id",
            "tool_use_id",
            "signature",
            "expires_at",
        ])),
        SchemaDefinition::sql_only(include_str!("../../schema/tool_call_ledger.sql")),
        SchemaDefinition::sql_only(include_str!("../../schema/trace_facts_ai.sql")),
        SchemaDefinition::new(
            "ai_usage_rollup_hourly",
            include_str!("../../schema/ai_usage_rollup_hourly.sql"),
        )
        .with_required_columns(columns(&["bucket", "user_id", "request_count"])),
        SchemaDefinition::new(
            "ai_context_rollup_hourly",
            include_str!("../../schema/ai_context_rollup_hourly.sql"),
        )
        .with_required_columns(columns(&["bucket", "context_id", "request_count"])),
        SchemaDefinition::new(
            "user_provider_credentials",
            include_str!("../../schema/user_provider_credentials.sql"),
        )
        .with_required_columns(columns(&["user_id", "credential", "ciphertext"])),
        SchemaDefinition::new(
            "ai_catalog_legacy_map",
            include_str!("../../schema/ai_catalog_legacy_map.sql"),
        )
        .with_required_columns(columns(&["legacy_provider", "credential"])),
    ]
}

fn columns(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}
