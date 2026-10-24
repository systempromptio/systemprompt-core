//! `Extension` registration for the authz subsystem: schema, migrations and
//! the `access_control` and `governance` config planes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::prelude::*;

use super::plane::AccessControlPlane;
use crate::policy::GovernancePlane;

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

    fn is_required(&self) -> bool {
        true
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        let mut schemas = access_control_schemas();
        schemas.extend(governance_schemas());
        schemas
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["users"]
    }

    fn migrations(&self) -> Vec<Migration> {
        extension_migrations!()
    }

    fn config_planes(&self) -> Vec<Arc<dyn ConfigPlane>> {
        vec![Arc::new(AccessControlPlane), Arc::new(GovernancePlane)]
    }
}

register_extension!(AuthzExtension);

fn access_control_schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new(
            "access_control_entities",
            include_str!("../../schema/access_control_entities.sql"),
        )
        .with_required_columns(vec![
            "entity_type".into(),
            "entity_id".into(),
            "default_included".into(),
            "source".into(),
        ]),
        SchemaDefinition::new(
            "access_control_rules",
            include_str!("../../schema/access_control_rules.sql"),
        )
        .with_required_columns(vec![
            "id".into(),
            "entity_type".into(),
            "entity_id".into(),
            "rule_type".into(),
            "rule_value".into(),
            "access".into(),
        ]),
        SchemaDefinition::sql_only(include_str!("../../schema/access_control_cache_notify.sql")),
        SchemaDefinition::sql_only(include_str!("../../schema/access_control_provenance.sql")),
        SchemaDefinition::new(
            "governance_config",
            include_str!("../../schema/governance_config.sql"),
        )
        .with_required_columns(vec!["entity".into(), "value".into(), "source".into()]),
        SchemaDefinition::sql_only(include_str!(
            "../../schema/governance_config_provenance.sql"
        )),
    ]
}

fn governance_schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new(
            "governance_decisions",
            include_str!("../../schema/governance_decisions.sql"),
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
        SchemaDefinition::new(
            "approval_requests",
            include_str!("../../schema/approval_requests.sql"),
        )
        .with_required_columns(vec![
            "call_id".into(),
            "tool_name".into(),
            "args_digest".into(),
            "requested_by".into(),
            "status".into(),
            "expires_at".into(),
        ]),
        SchemaDefinition::new(
            "governance_rate_windows",
            include_str!("../../schema/governance_rate_windows.sql"),
        )
        .with_required_columns(vec![
            "bucket_key".into(),
            "window_start".into(),
            "hits".into(),
            "expires_at".into(),
        ]),
        SchemaDefinition::new(
            "governance_rate_charges",
            include_str!("../../schema/governance_rate_charges.sql"),
        )
        .with_required_columns(vec![
            "call_id".into(),
            "bucket_key".into(),
            "position".into(),
            "expires_at".into(),
        ]),
    ]
}
