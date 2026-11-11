//! Validating and checking a management view without its unset variables.
//!
//! [`ManagedProfile::inspect`] builds the typed profile once, with every
//! unresolved value replaced by an empty value of its field's type, and runs
//! the derivation from `server.external_url` only when its inputs resolve.
//! That typed profile is never handed out: [`Inspection::validate`] runs each
//! profile rule whose inputs are all resolved and reports the others as not
//! evaluated, naming the variables they need; [`Inspection::evaluate`] does the
//! same for any other check given the key paths it reads. A deserialization
//! failure in a resolved value fails the inspection, exactly as it fails boot.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_yaml::Value;

use super::field_kind::profile_field_kind;
use super::validation::{DERIVE_INPUTS, RULES};
use super::{ManagedProfile, Profile, ProfileResult};
use crate::env::{FieldKind, InterpolationError, PathSegment, UNRESOLVED_TAG};

/// The rule results of a management view: errors from the rules that ran,
/// and the rules that did not run with the variables each one needs.
#[derive(Debug, Clone, Default)]
pub struct ValidationReport {
    pub errors: Vec<String>,
    pub not_evaluated: Vec<NotEvaluated>,
}

/// A rule or check that was not run because it reads an unresolved value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotEvaluated {
    pub name: &'static str,
    pub variables: Vec<String>,
}

/// The outcome of one check over a management view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evaluation<R> {
    Evaluated(R),
    NotEvaluated { variables: Vec<String> },
}

/// A management view with its typed profile built for rule evaluation.
#[derive(Debug)]
pub struct Inspection<'a> {
    managed: &'a ManagedProfile,
    probe: Profile,
}

impl ManagedProfile {
    pub fn inspect(&self) -> ProfileResult<Inspection<'_>> {
        let mut tree = self.tree().clone();
        replace_markers(&mut tree, &mut Vec::new());
        let mut probe = Profile::assemble(tree, self.path())?;
        if needed_variables(self.unresolved(), DERIVE_INPUTS).is_empty() {
            probe.derive_fields()?;
        }
        if needed_variables(self.unresolved(), &["secrets"]).is_empty()
            && let Some(secrets) = probe.secrets.as_ref()
        {
            secrets.validate()?;
        }
        Ok(Inspection {
            managed: self,
            probe,
        })
    }
}

impl Inspection<'_> {
    #[must_use]
    pub fn validate(&self) -> ValidationReport {
        let mut report = ValidationReport::default();
        for rule in RULES {
            let variables = needed_variables(self.managed.unresolved(), rule.inputs);
            if variables.is_empty() {
                (rule.check)(&self.probe, &mut report.errors);
            } else {
                report.not_evaluated.push(NotEvaluated {
                    name: rule.name,
                    variables,
                });
            }
        }
        report
    }

    pub fn evaluate<'s, R>(
        &'s self,
        inputs: &[&str],
        check: impl FnOnce(&'s Profile) -> R,
    ) -> Evaluation<R> {
        let variables = needed_variables(self.managed.unresolved(), inputs);
        if variables.is_empty() {
            Evaluation::Evaluated(check(&self.probe))
        } else {
            Evaluation::NotEvaluated { variables }
        }
    }
}

#[must_use]
pub fn needed_variables(unresolved: &[InterpolationError], inputs: &[&str]) -> Vec<String> {
    let mut variables: Vec<String> = unresolved
        .iter()
        .filter(|entry| inputs.iter().any(|input| reads(input, &entry.key_path)))
        .map(|entry| entry.variable.clone())
        .collect();
    variables.sort();
    variables.dedup();
    variables
}

fn reads(input: &str, key_path: &str) -> bool {
    key_path == input
        || key_path
            .strip_prefix(input)
            .is_some_and(|rest| rest.starts_with('.') || rest.starts_with('['))
}

fn replace_markers(value: &mut Value, path: &mut Vec<PathSegment>) {
    match value {
        Value::Tagged(tagged) if tagged.tag == UNRESOLVED_TAG => {
            *value = empty_of(profile_field_kind(path));
        },
        Value::Tagged(tagged) => replace_markers(&mut tagged.value, path),
        Value::Sequence(items) => {
            for (index, item) in items.iter_mut().enumerate() {
                path.push(PathSegment::Index(index));
                replace_markers(item, path);
                path.pop();
            }
        },
        Value::Mapping(map) => {
            for (key, item) in map.iter_mut() {
                let label = key.as_str().map_or_else(String::new, str::to_owned);
                path.push(PathSegment::Key(label));
                replace_markers(item, path);
                path.pop();
            }
        },
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {},
    }
}

fn empty_of(kind: FieldKind) -> Value {
    if kind.accepts(FieldKind::NULL) {
        Value::Null
    } else if kind.accepts_string() {
        Value::String(String::new())
    } else if kind.accepts(FieldKind::NUMBER) {
        Value::Number(0.into())
    } else if kind.accepts(FieldKind::BOOL) {
        Value::Bool(false)
    } else {
        Value::Null
    }
}
