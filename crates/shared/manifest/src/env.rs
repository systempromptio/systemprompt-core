//! Environment-variable reading and `${VAR}` / `${VAR:-default}` interpolation.
//!
//! Interpolation runs over a parsed YAML tree, never over document text: only
//! string scalars are rewritten, so a substituted value can never add a key,
//! end a scalar early or start a comment. A scalar that is exactly one
//! placeholder may resolve to a number or a boolean (so `port: ${PORT}` still
//! deserializes as an integer) and an empty resolution of such a scalar is
//! null; every other result stays a string. A placeholder whose variable is
//! unset (or empty) and has no `:-` default is an [`InterpolationError`]
//! naming the variable and the key path ([`interpolate_yaml`], for a profile
//! about to run), or is recorded and left in the tree as a typed
//! `!unresolved` marker ([`interpolate_yaml_available`], for a profile that is
//! only inspected, edited or shipped from a host that lacks the target's
//! variables).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::LazyLock;

use regex::Regex;
use serde_yaml::Value;
use thiserror::Error;

#[expect(
    clippy::expect_used,
    reason = "compile-time-constant regex; failure is a programmer bug, not runtime input"
)]
static INTERPOLATION_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\$\{([^}:]+)(?::-(.*?))?\}")
        .expect("INTERPOLATION_REGEX is a valid regex - this is a compile-time constant")
});

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("environment variable `{variable}` is not set and its placeholder has no `:-` default")]
pub struct UnsetVariable {
    pub variable: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error(
    "`{key_path}` references environment variable `{variable}`, which is not set; set it or \
     write `${{{variable}:-<default>}}`"
)]
pub struct InterpolationError {
    pub variable: String,
    pub key_path: String,
}

#[must_use]
pub fn read_env_optional(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(v) if !v.is_empty() => Some(v),
        Ok(_) | Err(_) => None,
    }
}

#[must_use]
pub fn none_if_blank(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.trim().is_empty())
}

#[must_use]
pub fn contains_placeholder(input: &str) -> bool {
    INTERPOLATION_REGEX.is_match(input)
}

pub fn interpolate(
    input: &str,
    lookup: &impl Fn(&str) -> Option<String>,
) -> Result<String, UnsetVariable> {
    let mut unset = None;
    let output = INTERPOLATION_REGEX.replace_all(input, |caps: &regex::Captures| {
        let var_name = &caps[1];
        lookup(var_name)
            .or_else(|| caps.get(2).map(|m| m.as_str().to_owned()))
            .unwrap_or_else(|| {
                unset.get_or_insert_with(|| var_name.to_owned());
                String::new()
            })
    });
    unset.map_or_else(
        || Ok(output.into_owned()),
        |variable| Err(UnsetVariable { variable }),
    )
}

pub fn interpolate_yaml(
    value: &mut Value,
    lookup: &impl Fn(&str) -> Option<String>,
) -> Result<(), InterpolationError> {
    walk(value, lookup, &mut String::new(), &mut |missing| {
        Err(missing)
    })
}

#[must_use]
pub fn interpolate_yaml_available(
    value: &mut Value,
    lookup: &impl Fn(&str) -> Option<String>,
) -> Vec<InterpolationError> {
    let mut unresolved = Vec::new();
    let outcome = walk(value, lookup, &mut String::new(), &mut |missing| {
        let marker = unresolved_marker(&missing.variable);
        unresolved.push(missing);
        Ok(marker)
    });
    match outcome {
        Ok(()) => unresolved,
        Err(missing) => vec![missing],
    }
}

pub const UNRESOLVED_TAG: &str = "!unresolved";

fn unresolved_marker(variable: &str) -> Value {
    Value::Tagged(Box::new(serde_yaml::value::TaggedValue {
        tag: serde_yaml::value::Tag::new(UNRESOLVED_TAG),
        value: Value::String(variable.to_owned()),
    }))
}

type OnUnset<'a> = dyn FnMut(InterpolationError) -> Result<Value, InterpolationError> + 'a;

fn walk(
    value: &mut Value,
    lookup: &impl Fn(&str) -> Option<String>,
    path: &mut String,
    on_unset: &mut OnUnset<'_>,
) -> Result<(), InterpolationError> {
    match value {
        Value::String(text) => {
            match resolve_scalar(text, lookup, path) {
                Ok(Some(resolved)) => *value = resolved,
                Ok(None) => {},
                Err(missing) => *value = on_unset(missing)?,
            }
            Ok(())
        },
        Value::Sequence(items) => {
            for (index, item) in items.iter_mut().enumerate() {
                let len = path.len();
                path.push_str(&format!("[{index}]"));
                walk(item, lookup, path, on_unset)?;
                path.truncate(len);
            }
            Ok(())
        },
        Value::Mapping(map) => {
            for (key, item) in map.iter_mut() {
                let len = path.len();
                if !path.is_empty() {
                    path.push('.');
                }
                path.push_str(&key_label(key));
                walk(item, lookup, path, on_unset)?;
                path.truncate(len);
            }
            Ok(())
        },
        Value::Tagged(tagged) => walk(&mut tagged.value, lookup, path, on_unset),
        Value::Null | Value::Bool(_) | Value::Number(_) => Ok(()),
    }
}

fn resolve_scalar(
    text: &str,
    lookup: &impl Fn(&str) -> Option<String>,
    path: &str,
) -> Result<Option<Value>, InterpolationError> {
    if !contains_placeholder(text) {
        return Ok(None);
    }
    let resolved =
        interpolate(text, lookup).map_err(|UnsetVariable { variable }| InterpolationError {
            variable,
            key_path: if path.is_empty() {
                "<root>".to_owned()
            } else {
                path.to_owned()
            },
        })?;
    if !is_single_placeholder(text) {
        return Ok(Some(Value::String(resolved)));
    }
    if resolved.is_empty() {
        return Ok(Some(Value::Null));
    }
    Ok(Some(match serde_yaml::from_str::<Value>(&resolved) {
        Ok(typed @ (Value::Bool(_) | Value::Number(_))) => typed,
        _ => Value::String(resolved),
    }))
}

fn is_single_placeholder(text: &str) -> bool {
    INTERPOLATION_REGEX
        .find(text)
        .is_some_and(|m| m.start() == 0 && m.end() == text.len())
}

fn key_label(key: &Value) -> String {
    match key {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        other => serde_yaml::to_string(other)
            .map(|s| s.trim_end().to_owned())
            .unwrap_or_default(),
    }
}
