//! Compile-time deterministic verification supplied by an evaluation template.
//!
//! Core owns evidence integrity and the five required result categories. A
//! template owns the meaning of its authored assertion names and registers one
//! verifier in the final binary. Missing or ambiguous registration fails every
//! deterministic category closed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use super::execution::EvidenceArchive;
use super::resources::CaseContent;

#[derive(Debug, Clone)]
pub struct VerificationInput<'a> {
    pub case: &'a CaseContent,
    pub evidence: &'a EvidenceArchive,
}

#[derive(Debug, Clone, Default)]
pub struct VerificationResult {
    pub hard_failures: Vec<String>,
    pub checks: BTreeMap<String, bool>,
}

impl VerificationResult {
    pub fn fail_closed(reason: impl Into<String>) -> Self {
        Self {
            hard_failures: vec![reason.into()],
            checks: [
                ("arithmetic".to_owned(), false),
                ("permissions".to_owned(), false),
                ("evidence_references".to_owned(), false),
                ("install_integrity".to_owned(), false),
                ("write_readbacks".to_owned(), false),
            ]
            .into_iter()
            .collect(),
        }
    }
}

pub trait DeterministicEvaluator: Sync {
    fn id(&self) -> &'static str;
    fn supports(&self, case: &CaseContent) -> bool;
    fn evaluate(&self, input: VerificationInput<'_>) -> VerificationResult;
}

inventory::collect!(&'static dyn DeterministicEvaluator);

pub fn evaluate(input: VerificationInput<'_>) -> VerificationResult {
    let mut matching = inventory::iter::<&'static dyn DeterministicEvaluator>
        .into_iter()
        .filter(|evaluator| evaluator.supports(input.case));
    let Some(evaluator) = matching.next() else {
        return VerificationResult::fail_closed("No deterministic evaluator supports this case");
    };
    if matching.next().is_some() {
        return VerificationResult::fail_closed(
            "More than one deterministic evaluator supports this case",
        );
    }
    let result = evaluator.evaluate(input);
    let required = [
        "arithmetic",
        "permissions",
        "evidence_references",
        "install_integrity",
        "write_readbacks",
    ];
    if result.checks.len() != required.len()
        || required
            .iter()
            .any(|name| !result.checks.contains_key(*name))
    {
        return VerificationResult::fail_closed(format!(
            "Deterministic evaluator {} omitted a required check",
            evaluator.id()
        ));
    }
    result
}
