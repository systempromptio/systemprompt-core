use systemprompt_extension::prelude::*;

#[derive(Debug, Clone, Copy, Default)]
pub struct AuthzExtension;

impl Extension for AuthzExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "authz",
            name: "Authorization",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn migration_weight(&self) -> u32 {
        110
    }

    fn is_required(&self) -> bool {
        true
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![
            SchemaDefinition::new(
                "access_control_rules",
                include_str!("schema/access_control_rules.sql"),
            )
            .with_required_columns(vec![
                "id".into(),
                "entity_type".into(),
                "entity_id".into(),
                "rule_type".into(),
                "rule_value".into(),
                "access".into(),
            ]),
            SchemaDefinition::new(
                "governance_decisions",
                include_str!("schema/governance_decisions.sql"),
            )
            .with_required_columns(vec![
                "id".into(),
                "user_id".into(),
                "session_id".into(),
                "tool_name".into(),
                "decision".into(),
                "policy".into(),
                "reason".into(),
            ]),
        ]
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["users"]
    }
}

register_extension!(AuthzExtension);
