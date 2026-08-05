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
            SchemaDefinition::new("eval_runs", include_str!("../schema/eval_runs.sql"))
                .with_required_columns(vec!["id".into(), "kind".into(), "status".into()]),
            SchemaDefinition::new("eval_cases", include_str!("../schema/eval_cases.sql"))
                .with_required_columns(vec!["id".into(), "name".into(), "prompt_body".into()]),
            SchemaDefinition::new("eval_results", include_str!("../schema/eval_results.sql"))
                .with_required_columns(vec!["id".into(), "run_id".into(), "verdict".into()]),
            SchemaDefinition::new("eval_pairs", include_str!("../schema/eval_pairs.sql"))
                .with_required_columns(vec!["id".into(), "run_id".into(), "winner".into()]),
            SchemaDefinition::new(
                "eval_judge_calls",
                include_str!("../schema/eval_judge_calls.sql"),
            )
            .with_required_columns(vec!["conversation_id".into()]),
            SchemaDefinition::new("eval_rubrics", include_str!("../schema/eval_rubrics.sql"))
                .with_required_columns(vec!["id".into(), "name".into()]),
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
