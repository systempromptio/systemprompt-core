//! Which calls `require_approval` holds, and on what.
//!
//! A rule is a tool-name match, optionally narrowed by conditions on the call's
//! own arguments. Without conditions it is the original behaviour: name
//! matches, call holds. With them the hold is reserved for the calls that
//! actually warrant a human — an `email_send` leaving our domain rather than
//! every `email_send`.
//!
//! The two failure directions are deliberately opposite, and the reason is
//! worth stating because it looks inconsistent otherwise. A rule that fails to
//! *parse* is dropped: a config typo must never conjure a hold nobody
//! configured, which is the same posture as `patterns` defaulting to empty. A
//! rule that parses but cannot be *evaluated* — a path that addresses nothing,
//! a number compared against a string — holds: at that point an operator has
//! declared the field decides safety, and the honest answer to "I cannot tell"
//! from an escalation policy is to ask the human.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;
use serde_yaml::Value as YamlValue;

use super::operators::{Op, erase_indices};
use crate::policy::governed::GovernedScalar;

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum Quantifier {
    #[default]
    Any,
    All,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RuleSpec {
    Bare(String),
    Conditional {
        tool: String,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        when: Vec<ConditionSpec>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConditionSpec {
    path: String,
    op: Op,
    #[serde(default)]
    value: Option<YamlValue>,
    #[serde(default)]
    values: Vec<YamlValue>,
    #[serde(default)]
    negate: bool,
    #[serde(default, rename = "match")]
    quantifier: Quantifier,
}

#[derive(Debug)]
pub(super) struct Rule {
    tool: String,
    name: Option<String>,
    conditions: Vec<Condition>,
}

#[derive(Debug)]
struct Condition {
    path: String,
    op: Op,
    strings: Vec<String>,
    number: Option<f64>,
    negate: bool,
    quantifier: Quantifier,
}

pub(super) enum Verdict {
    Hold(String),
    Pass,
}

impl Rule {
    pub(super) fn matches_tool(&self, tool: &str) -> bool {
        tool.contains(self.tool.as_str())
    }

    fn label(&self, detail: &str) -> String {
        self.name.as_ref().map_or_else(
            || format!("{}: {detail}", self.tool),
            |name| format!("{} [{name}]: {detail}", self.tool),
        )
    }

    pub(super) fn evaluate(&self, scalars: &[GovernedScalar<'_>]) -> Verdict {
        if self.conditions.is_empty() {
            return Verdict::Hold(self.tool.clone());
        }
        for condition in &self.conditions {
            match condition.evaluate(scalars) {
                ConditionOutcome::Met(detail) => return Verdict::Hold(self.label(&detail)),
                ConditionOutcome::Unresolved(detail) => {
                    return Verdict::Hold(self.label(&format!("{detail} (fail-closed)")));
                },
                ConditionOutcome::NotMet => {},
            }
        }
        Verdict::Pass
    }
}

enum ConditionOutcome {
    Met(String),
    NotMet,
    Unresolved(String),
}

impl Condition {
    fn describe(&self, path: &str) -> String {
        let negation = if self.negate { "not " } else { "" };
        let operand = self.number.map_or_else(
            || {
                self.strings
                    .iter()
                    .map(|s| format!("{s:?}"))
                    .collect::<Vec<_>>()
                    .join(" | ")
            },
            |n| n.to_string(),
        );
        if self.op == Op::Exists {
            return format!("{path} {negation}exists");
        }
        format!("{path} {negation}{} {operand}", self.op.label())
    }

    fn evaluate(&self, scalars: &[GovernedScalar<'_>]) -> ConditionOutcome {
        let candidates: Vec<&GovernedScalar<'_>> = scalars
            .iter()
            .filter(|scalar| erase_indices(&scalar.path) == self.path)
            .collect();

        // Why: an empty candidate set is not vacuous truth. `to: []` must not
        // read as "every recipient is internal" under `match: all`.
        if candidates.is_empty() {
            return ConditionOutcome::Unresolved(format!("{} unresolved", self.path));
        }

        let mut hit: Option<String> = None;
        let mut all_hit = true;
        for candidate in candidates {
            let Some(raw) = self.op.test(candidate.value, &self.strings, self.number) else {
                return ConditionOutcome::Unresolved(format!(
                    "{} not comparable with {}",
                    candidate.path,
                    self.op.label()
                ));
            };
            if raw == self.negate {
                all_hit = false;
            } else if hit.is_none() {
                hit = Some(self.describe(&candidate.path));
            }
        }

        match self.quantifier {
            Quantifier::Any => hit.map_or(ConditionOutcome::NotMet, ConditionOutcome::Met),
            Quantifier::All => {
                if all_hit {
                    hit.map_or(ConditionOutcome::NotMet, ConditionOutcome::Met)
                } else {
                    ConditionOutcome::NotMet
                }
            },
        }
    }
}


pub(super) fn compile(v: &YamlValue) -> Vec<Rule> {
    v.get("patterns")
        .and_then(YamlValue::as_sequence)
        .map(|seq| seq.iter().filter_map(compile_one).collect())
        .unwrap_or_default()
}

fn compile_one(entry: &YamlValue) -> Option<Rule> {
    match serde_yaml::from_value::<RuleSpec>(entry.clone()) {
        Ok(RuleSpec::Bare(tool)) => Some(Rule {
            tool,
            name: None,
            conditions: Vec::new(),
        }),
        Ok(RuleSpec::Conditional { tool, name, when }) => {
            let declared = when.len();
            let conditions = when
                .into_iter()
                .filter_map(|spec| compile_condition(&tool, spec))
                .collect::<Vec<_>>();
            // Why: an empty condition list means "hold every call to this tool"
            // (RuleSpec::Bare). A conditional whose conditions all failed to
            // compile would reach that same state and hold everything — the
            // opposite of the posture this module argues for, where a config
            // typo must never conjure a hold nobody configured. Drop it.
            if declared > 0 && conditions.is_empty() {
                tracing::error!(
                    policy = super::ID,
                    tool = %tool,
                    declared,
                    "every condition on this rule failed to compile; dropping the rule \
                     rather than holding every call to the tool"
                );
                return None;
            }
            Some(Rule {
                tool,
                name,
                conditions,
            })
        },
        Err(error) => {
            tracing::error!(
                %error,
                policy = super::ID,
                "malformed require_approval patterns entry — ignoring it"
            );
            None
        },
    }
}

fn compile_condition(tool: &str, spec: ConditionSpec) -> Option<Condition> {
    let mut literals = spec.values;
    if let Some(single) = spec.value {
        literals.insert(0, single);
    }
    let number = literals.first().and_then(YamlValue::as_f64);
    let strings: Vec<String> = literals
        .iter()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect();

    let usable = match spec.op {
        Op::Exists => true,
        op if op.is_numeric() => number.is_some(),
        _ => !strings.is_empty(),
    };
    if !usable {
        tracing::error!(
            tool,
            path = spec.path,
            op = spec.op.label(),
            policy = super::ID,
            "require_approval condition has no operand its operator can use — ignoring it"
        );
        return None;
    }
    Some(Condition {
        path: spec.path,
        op: spec.op,
        strings,
        number,
        negate: spec.negate,
        quantifier: spec.quantifier,
    })
}
