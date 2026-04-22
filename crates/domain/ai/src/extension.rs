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

    fn migration_weight(&self) -> u32 {
        35
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![
            SchemaDefinition::inline("ai_requests", include_str!("../schema/ai_requests.sql"))
                .with_required_columns(vec![
                    "id".into(),
                    "provider".into(),
                    "model".into(),
                    "created_at".into(),
                ]),
            SchemaDefinition::inline(
                "ai_request_messages",
                include_str!("../schema/ai_request_messages.sql"),
            )
            .with_required_columns(vec![
                "id".into(),
                "request_id".into(),
                "role".into(),
            ]),
            SchemaDefinition::inline(
                "ai_request_tool_calls",
                include_str!("../schema/ai_request_tool_calls.sql"),
            )
            .with_required_columns(vec![
                "id".into(),
                "request_id".into(),
                "tool_name".into(),
            ]),
            SchemaDefinition::inline(
                "ai_request_payloads",
                include_str!("../schema/ai_request_payloads.sql"),
            )
            .with_required_columns(vec!["ai_request_id".into()]),
            SchemaDefinition::inline(
                "ai_safety_findings",
                include_str!("../schema/ai_safety_findings.sql"),
            )
            .with_required_columns(vec![
                "id".into(),
                "ai_request_id".into(),
                "severity".into(),
            ]),
            SchemaDefinition::inline(
                "ai_quota_buckets",
                include_str!("../schema/ai_quota_buckets.sql"),
            )
            .with_required_columns(vec![
                "id".into(),
                "user_id".into(),
                "window_start".into(),
            ]),
            SchemaDefinition::inline(
                "ai_gateway_policies",
                include_str!("../schema/ai_gateway_policies.sql"),
            )
            .with_required_columns(vec!["id".into(), "name".into(), "spec".into()]),
        ]
    }

    fn migrations(&self) -> Vec<Migration> {
        vec![Migration::new(
            1,
            "gateway_governance",
            include_str!("../schema/migrations/001_gateway_governance.sql"),
        )]
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["users", "mcp"]
    }
}

register_extension!(AiExtension);
