//! Extension registration — wires the evaluation schemas (runs, cases,
//! results, pairs, judge calls, rubrics) and their reconcile migrations into
//! the extension framework.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

#[derive(Debug, Clone, Copy, Default)]
pub struct EvaluationExtension;

impl Extension for EvaluationExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "evaluation",
            name: "Evaluation",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        let mut schemas = campaign_schemas();
        schemas.extend(experiment_schemas());
        schemas.extend(measurement_schemas());
        schemas.extend(evidence_schemas());
        schemas
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["ai"]
    }

    fn migrations(&self) -> Vec<Migration> {
        extension_migrations!()
    }
}

register_extension!(EvaluationExtension);

fn campaign_schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new(
            "eval_campaigns",
            include_str!("../schema/eval_campaigns.sql"),
        ),
        SchemaDefinition::new(
            "eval_campaign_experiments",
            include_str!("../schema/eval_campaign_experiments.sql"),
        ),
        SchemaDefinition::new(
            "eval_campaign_events",
            include_str!("../schema/eval_campaign_events.sql"),
        ),
        SchemaDefinition::new(
            "eval_campaign_diagnostics",
            include_str!("../schema/eval_campaign_diagnostics.sql"),
        ),
        SchemaDefinition::new(
            "eval_campaign_holdout_proposals",
            include_str!("../schema/eval_campaign_holdout_proposals.sql"),
        ),
        SchemaDefinition::new(
            "eval_holdout_content_consumption",
            include_str!("../schema/eval_holdout_content_consumption.sql"),
        ),
    ]
}

fn experiment_schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new(
            "eval_resource_revisions",
            include_str!("../schema/eval_resource_revisions.sql"),
        ),
        SchemaDefinition::new(
            "eval_budget_accounts",
            include_str!("../schema/eval_budget_accounts.sql"),
        ),
        SchemaDefinition::new(
            "eval_budget_reservations",
            include_str!("../schema/eval_budget_reservations.sql"),
        ),
        SchemaDefinition::new(
            "eval_experiments",
            include_str!("../schema/eval_experiments.sql"),
        ),
        SchemaDefinition::new(
            "eval_executions",
            include_str!("../schema/eval_executions.sql"),
        ),
        SchemaDefinition::new(
            "eval_execution_events",
            include_str!("../schema/eval_execution_events.sql"),
        ),
        SchemaDefinition::new(
            "eval_execution_approvals",
            include_str!("../schema/eval_execution_approvals.sql"),
        ),
        SchemaDefinition::new(
            "eval_execution_cleanup",
            include_str!("../schema/eval_execution_cleanup.sql"),
        ),
        SchemaDefinition::new(
            "eval_approved_operation_receipts",
            include_str!("../schema/eval_approved_operation_receipts.sql"),
        ),
    ]
}

fn measurement_schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new(
            "eval_execution_measurements",
            include_str!("../schema/eval_execution_measurements.sql"),
        ),
        SchemaDefinition::new(
            "eval_suggestions",
            include_str!("../schema/eval_suggestions.sql"),
        ),
        SchemaDefinition::new(
            "eval_holdout_consumption",
            include_str!("../schema/eval_holdout_consumption.sql"),
        ),
        SchemaDefinition::new(
            "eval_fixture_payloads",
            include_str!("../schema/eval_fixture_payloads.sql"),
        ),
        SchemaDefinition::new(
            "eval_fixture_test_records",
            include_str!("../schema/eval_fixture_test_records.sql"),
        ),
        SchemaDefinition::new(
            "eval_managed_workspace_projections",
            include_str!("../schema/eval_managed_workspace_projections.sql"),
        ),
        SchemaDefinition::new(
            "eval_managed_workspace_assets",
            include_str!("../schema/eval_managed_workspace_assets.sql"),
        ),
    ]
}

fn evidence_schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new(
            "eval_execution_evidence",
            include_str!("../schema/eval_execution_evidence.sql"),
        ),
        SchemaDefinition::new(
            "eval_session_bindings",
            include_str!("../schema/eval_session_bindings.sql"),
        ),
        SchemaDefinition::new(
            "eval_request_reservations",
            include_str!("../schema/eval_request_reservations.sql"),
        ),
        SchemaDefinition::new("eval_workers", include_str!("../schema/eval_workers.sql")),
        SchemaDefinition::new(
            "eval_execution_artifacts",
            include_str!("../schema/eval_execution_artifacts.sql"),
        ),
        SchemaDefinition::new(
            "eval_execution_capabilities",
            include_str!("../schema/eval_execution_capabilities.sql"),
        ),
        SchemaDefinition::new("eval_cases", include_str!("../schema/eval_cases.sql"))
            .with_required_columns(vec!["id".into(), "name".into(), "prompt_body".into()]),
    ]
}
