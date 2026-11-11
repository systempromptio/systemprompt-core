//! Environment-variable reading and `${VAR}` / `${VAR:-default}` interpolation.
//!
//! Interpolation runs over a parsed YAML tree, never over document text: only
//! string scalars are rewritten, so a substituted value can never add a key,
//! end a scalar early or start a comment. How a scalar that is exactly one
//! placeholder enters the tree is decided by the type of the field it lands
//! in ([`FieldKind`]), never by the value's appearance: a field that accepts a
//! string receives the value as a string, a numeric or boolean field receives
//! it parsed, and an empty value is null where the field accepts null. A
//! placeholder embedded in longer text always yields a string. A placeholder
//! whose variable is unset (or empty) and has no `:-` default is an
//! [`InterpolationError`] naming the variable and the key path
//! ([`interpolate_yaml`], for a profile about to run), or is recorded and left
//! in the tree as a typed `!unresolved` marker ([`interpolate_yaml_available`],
//! for a profile that is only inspected, edited or shipped from a host that
//! lacks the target's variables).
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

/// The scalar types a key path accepts, as a set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldKind(u8);

impl FieldKind {
    pub const NONE: Self = Self(0);
    pub const STRING: Self = Self(1);
    pub const NUMBER: Self = Self(2);
    pub const BOOL: Self = Self(4);
    pub const NULL: Self = Self(8);
    pub const UNKNOWN: Self = Self(16);

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn accepts(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    #[must_use]
    pub const fn accepts_string(self) -> bool {
        self.accepts(Self::STRING) || self.accepts(Self::UNKNOWN)
    }
}

/// One step of a key path: a mapping key or a sequence index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathSegment {
    Key(String),
    Index(usize),
}

pub type FieldKindLookup<'a> = &'a dyn Fn(&[PathSegment]) -> FieldKind;

#[must_use]
pub const fn untyped(_path: &[PathSegment]) -> FieldKind {
    FieldKind::UNKNOWN
}

pub fn interpolate_yaml(
    value: &mut Value,
    lookup: &impl Fn(&str) -> Option<String>,
    kinds: FieldKindLookup<'_>,
) -> Result<(), InterpolationError> {
    walk(value, lookup, kinds, &mut Vec::new(), &mut |missing| {
        Err(missing)
    })
}

#[must_use]
pub fn interpolate_yaml_available(
    value: &mut Value,
    lookup: &impl Fn(&str) -> Option<String>,
    kinds: FieldKindLookup<'_>,
) -> Vec<InterpolationError> {
    let mut unresolved = Vec::new();
    let outcome = walk(value, lookup, kinds, &mut Vec::new(), &mut |missing| {
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

#[must_use]
pub fn key_path_label(path: &[PathSegment]) -> String {
    let mut label = String::new();
    for segment in path {
        match segment {
            PathSegment::Key(key) => {
                if !label.is_empty() {
                    label.push('.');
                }
                label.push_str(key);
            },
            PathSegment::Index(index) => label.push_str(&format!("[{index}]")),
        }
    }
    if label.is_empty() {
        "<root>".to_owned()
    } else {
        label
    }
}

type OnUnset<'a> = dyn FnMut(InterpolationError) -> Result<Value, InterpolationError> + 'a;

fn walk(
    value: &mut Value,
    lookup: &impl Fn(&str) -> Option<String>,
    kinds: FieldKindLookup<'_>,
    path: &mut Vec<PathSegment>,
    on_unset: &mut OnUnset<'_>,
) -> Result<(), InterpolationError> {
    match value {
        Value::String(text) => {
            match resolve_scalar(text, lookup, kinds, path) {
                Ok(Some(resolved)) => *value = resolved,
                Ok(None) => {},
                Err(missing) => *value = on_unset(missing)?,
            }
            Ok(())
        },
        Value::Sequence(items) => {
            for (index, item) in items.iter_mut().enumerate() {
                path.push(PathSegment::Index(index));
                let outcome = walk(item, lookup, kinds, path, on_unset);
                path.pop();
                outcome?;
            }
            Ok(())
        },
        Value::Mapping(map) => {
            for (key, item) in map.iter_mut() {
                path.push(PathSegment::Key(key_label(key)));
                let outcome = walk(item, lookup, kinds, path, on_unset);
                path.pop();
                outcome?;
            }
            Ok(())
        },
        Value::Tagged(tagged) => walk(&mut tagged.value, lookup, kinds, path, on_unset),
        Value::Null | Value::Bool(_) | Value::Number(_) => Ok(()),
    }
}

fn resolve_scalar(
    text: &str,
    lookup: &impl Fn(&str) -> Option<String>,
    kinds: FieldKindLookup<'_>,
    path: &[PathSegment],
) -> Result<Option<Value>, InterpolationError> {
    if !contains_placeholder(text) {
        return Ok(None);
    }
    let resolved =
        interpolate(text, lookup).map_err(|UnsetVariable { variable }| InterpolationError {
            variable,
            key_path: key_path_label(path),
        })?;
    if !is_single_placeholder(text) {
        return Ok(Some(Value::String(resolved)));
    }
    Ok(Some(typed_scalar(resolved, kinds(path))))
}

fn typed_scalar(resolved: String, kind: FieldKind) -> Value {
    if resolved.is_empty() && kind.accepts(FieldKind::NULL) {
        return Value::Null;
    }
    if kind.accepts_string() {
        return Value::String(resolved);
    }
    if kind.accepts(FieldKind::BOOL) {
        match resolved.as_str() {
            "true" => return Value::Bool(true),
            "false" => return Value::Bool(false),
            _ => {},
        }
    }
    if kind.accepts(FieldKind::NUMBER) {
        if let Ok(n) = resolved.parse::<i64>() {
            return Value::Number(n.into());
        }
        if let Ok(n) = resolved.parse::<u64>() {
            return Value::Number(n.into());
        }
        if let Ok(n) = resolved.parse::<f64>() {
            return Value::Number(n.into());
        }
    }
    Value::String(resolved)
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
