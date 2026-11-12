//! The YAML-tree walk behind `${VAR}` interpolation and its three unset
//! policies: fail, mark `!unresolved`, or keep the placeholder text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_yaml::Value;

use super::{
    FieldKind, FieldKindLookup, INTERPOLATION_REGEX, InterpolationError, PathSegment,
    UNRESOLVED_TAG, UnsetVariable, contains_placeholder, interpolate, key_path_label,
};

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
        Ok(Some(marker))
    });
    match outcome {
        Ok(()) => unresolved,
        Err(missing) => vec![missing],
    }
}

#[must_use]
pub fn interpolate_yaml_keeping_unresolved(
    value: &mut Value,
    lookup: &impl Fn(&str) -> Option<String>,
    kinds: FieldKindLookup<'_>,
) -> Vec<InterpolationError> {
    let mut unresolved = Vec::new();
    let outcome = walk(value, lookup, kinds, &mut Vec::new(), &mut |missing| {
        unresolved.push(missing);
        Ok(None)
    });
    match outcome {
        Ok(()) => unresolved,
        Err(missing) => vec![missing],
    }
}

fn unresolved_marker(variable: &str) -> Value {
    Value::Tagged(Box::new(serde_yaml::value::TaggedValue {
        tag: serde_yaml::value::Tag::new(UNRESOLVED_TAG),
        value: Value::String(variable.to_owned()),
    }))
}

type OnUnset<'a> = dyn FnMut(InterpolationError) -> Result<Option<Value>, InterpolationError> + 'a;

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
                Err(missing) => {
                    if let Some(replacement) = on_unset(missing)? {
                        *value = replacement;
                    }
                },
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
    let kind = kinds(path);
    if kind == FieldKind::VERBATIM {
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
    Ok(Some(typed_scalar(resolved, kind)))
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
