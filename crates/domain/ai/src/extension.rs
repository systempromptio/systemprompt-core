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
        20
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
            .with_required_columns(vec!["id".into(), "request_id".into(), "role".into()]),
            SchemaDefinition::inline(
                "ai_request_tool_calls",
                include_str!("../schema/ai_request_tool_calls.sql"),
            )
            .with_required_columns(vec!["id".into(), "request_id".into(), "tool_name".into()]),
        ]
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["users"]
    }
}

register_extension!(AiExtension);
