//! `Extension` registration for the AI domain: schemas, migrations,
//! dependencies.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

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

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![
            SchemaDefinition::sql_only(include_str!("../schema/reporting_privacy.sql")),
            SchemaDefinition::sql_only(include_str!("../schema/reporting_capture.sql")),
            SchemaDefinition::new("ai_requests", include_str!("../schema/ai_requests.sql"))
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
                ])),
            SchemaDefinition::new(
                "ai_request_client_evidence",
                include_str!("../schema/ai_request_client_evidence.sql"),
            )
            .with_required_columns(columns(&["ai_request_id", "kind_source"])),
            SchemaDefinition::new(
                "ai_request_messages",
                include_str!("../schema/ai_request_messages.sql"),
            )
            .with_required_columns(columns(&["id", "request_id", "role"])),
            SchemaDefinition::new(
                "ai_request_tool_calls",
                include_str!("../schema/ai_request_tool_calls.sql"),
            )
            .with_required_columns(columns(&["id", "request_id", "tool_name"])),
            SchemaDefinition::new(
                "ai_request_payloads",
                include_str!("../schema/ai_request_payloads.sql"),
            )
            .with_required_columns(columns(&["ai_request_id"])),
            SchemaDefinition::new(
                "ai_safety_findings",
                include_str!("../schema/ai_safety_findings.sql"),
            )
            .with_required_columns(columns(&["id", "ai_request_id", "severity"])),
            SchemaDefinition::new(
                "ai_quota_buckets",
                include_str!("../schema/ai_quota_buckets.sql"),
            )
            .with_required_columns(columns(&[
                "id",
                "subject_kind",
                "subject_id",
                "window_start",
            ])),
            SchemaDefinition::new(
                "ai_gateway_policies",
                include_str!("../schema/ai_gateway_policies.sql"),
            )
            .with_required_columns(columns(&["id", "name", "spec"])),
            SchemaDefinition::new(
                "ai_gateway_thought_signatures",
                include_str!("../schema/ai_gateway_thought_signatures.sql"),
            )
            .with_required_columns(columns(&[
                "conversation_id",
                "tool_use_id",
                "signature",
                "expires_at",
            ])),
            SchemaDefinition::sql_only(include_str!("../schema/tool_call_ledger.sql")),
        ]
    }

    fn migrations(&self) -> Vec<Migration> {
        extension_migrations!()
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["users", "mcp"]
    }
}

register_extension!(AiExtension);

fn columns(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}
