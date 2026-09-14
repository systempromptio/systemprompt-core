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
        vec![
            SchemaDefinition::new("eval_campaigns", include_str!("../schema/campaigns.sql")),
            SchemaDefinition::new(
                "eval_experiments",
                include_str!("../schema/experiments.sql"),
            ),
            SchemaDefinition::new(
                "eval_execution_evidence",
                include_str!("../schema/execution_evidence.sql"),
            ),
            SchemaDefinition::new("eval_cases", include_str!("../schema/eval_cases.sql"))
                .with_required_columns(vec!["id".into(), "name".into(), "prompt_body".into()]),
            SchemaDefinition::new(
                "eval_campaign_completion",
                include_str!("../schema/campaign_completion.sql"),
            ),
        ]
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["ai"]
    }

    fn migrations(&self) -> Vec<Migration> {
        extension_migrations!()
    }
}

register_extension!(EvaluationExtension);
