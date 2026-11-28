//! Where two normalised traces part: one [`Difference`] per leaf that differs,
//! or per node whose shape or step kind differs.
//!
//! Placeholders in the expected side (`{{protocol}}`, `{{command.<name>}}`)
//! are substituted first, and a `digest` that either side gives as `null` is
//! not compared (the corpus's rule for machine-dependent text).

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Difference {
    pub path: String,
    pub expected: Value,
    pub actual: Value,
}

pub fn substitute(value: &Value, substitutions: &[(String, String)]) -> Value {
    match value {
        Value::String(text) => {
            let mut replaced = text.clone();
            for (placeholder, with) in substitutions {
                replaced = replaced.replace(placeholder.as_str(), with);
            }
            Value::String(replaced)
        },
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| substitute(item, substitutions))
                .collect(),
        ),
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(key, item)| (key.clone(), substitute(item, substitutions)))
                .collect(),
        ),
        _ => value.clone(),
    }
}

pub fn differences(
    expected: &Value,
    actual: &Value,
    substitutions: &[(String, String)],
) -> Vec<Difference> {
    let expected = substitute(expected, substitutions);
    let mut found = Vec::new();
    walk("", &expected, actual, &mut found);
    found
}

fn walk(path: &str, expected: &Value, actual: &Value, found: &mut Vec<Difference>) {
    match (expected, actual) {
        (Value::Object(left), Value::Object(right)) => {
            if left.get("kind").is_some() && left.get("kind") != right.get("kind") {
                push(path, expected, actual, found);
                return;
            }
            let mut keys: Vec<&String> = left.keys().chain(right.keys()).collect();
            keys.sort();
            keys.dedup();
            for key in keys {
                let (one, other) = (left.get(key), right.get(key));
                if key == "digest" && (one == Some(&Value::Null) || other == Some(&Value::Null)) {
                    continue;
                }
                let child = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                walk(
                    &child,
                    one.unwrap_or(&Value::Null),
                    other.unwrap_or(&Value::Null),
                    found,
                );
            }
        },
        (Value::Array(left), Value::Array(right)) if !left.iter().chain(right).all(is_scalar) => {
            for at in 0..left.len().max(right.len()) {
                let child = format!("{path}[{at}]");
                walk(
                    &child,
                    left.get(at).unwrap_or(&Value::Null),
                    right.get(at).unwrap_or(&Value::Null),
                    found,
                );
            }
        },
        _ if expected != actual => push(path, expected, actual, found),
        _ => {},
    }
}

const fn is_scalar(value: &Value) -> bool {
    !matches!(value, Value::Array(_) | Value::Object(_))
}

fn push(path: &str, expected: &Value, actual: &Value, found: &mut Vec<Difference>) {
    found.push(Difference {
        path: path.to_owned(),
        expected: expected.clone(),
        actual: actual.clone(),
    });
}
