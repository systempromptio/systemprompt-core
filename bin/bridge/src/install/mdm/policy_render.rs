//! Platform renderers for the Claude Desktop managed policy: Windows registry
//! values and macOS plist bodies.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::policy::{PolicyEntry, PolicyValue};
use crate::install::xml;

#[must_use]
pub fn reg_values(policy: &[PolicyEntry]) -> Vec<(&'static str, &'static str, String)> {
    policy
        .iter()
        .map(|(name, value)| (*name, "REG_SZ", reg_encode(value)))
        .collect()
}

// Why: Claude's registry encoding requires strings, with arrays and objects
// encoded as JSON text.
fn reg_encode(value: &PolicyValue) -> String {
    match value {
        PolicyValue::Str(s) => s.clone(),
        PolicyValue::Bool(b) => b.to_string(),
        PolicyValue::Json(v) => v.to_string(),
    }
}

#[must_use]
pub fn plist_body(policy: &[PolicyEntry], indent: &str) -> String {
    let mut out = String::new();
    for (name, value) in policy {
        out.push_str(&format!("{indent}<key>{}</key>\n", xml::escape(name)));
        out.push_str(&plist_value(value, indent));
    }
    out
}

// Why: Claude's published preference encoding specifies string booleans for
// the top-level policy keys, not plist booleans.
fn plist_value(value: &PolicyValue, indent: &str) -> String {
    match value {
        PolicyValue::Str(s) => format!("{indent}<string>{}</string>\n", xml::escape(s)),
        PolicyValue::Bool(b) => format!("{indent}<string>{b}</string>\n"),
        PolicyValue::Json(v) => plist_json(v, indent),
    }
}

// Why: inside an `object[]`/`dict` value Claude Desktop reads the native plist
// as the equivalent JSON and validates each entry against the key's schema. A
// field typed boolean (`allowedWorkspaceFolders[].isDefaultSelected`) written
// as a string is a malformed entry, the entry is dropped, and an empty
// resulting list blocks the Code tab from adding any folder.
// JSON: Claude Desktop managed policy — JSON rendered to the equivalent native
// plist.
fn plist_json(value: &serde_json::Value, indent: &str) -> String {
    let inner = format!("{indent}  ");
    match value {
        serde_json::Value::Null => format!("{indent}<string></string>\n"),
        serde_json::Value::Bool(true) => format!("{indent}<true/>\n"),
        serde_json::Value::Bool(false) => format!("{indent}<false/>\n"),
        serde_json::Value::Number(n) if n.is_i64() || n.is_u64() => {
            format!("{indent}<integer>{n}</integer>\n")
        },
        serde_json::Value::Number(n) => format!("{indent}<real>{n}</real>\n"),
        serde_json::Value::String(s) => {
            format!("{indent}<string>{}</string>\n", xml::escape(s))
        },
        serde_json::Value::Array(items) => {
            let mut out = format!("{indent}<array>\n");
            for item in items {
                out.push_str(&plist_json(item, &inner));
            }
            out.push_str(&format!("{indent}</array>\n"));
            out
        },
        serde_json::Value::Object(map) => {
            let mut out = format!("{indent}<dict>\n");
            for (k, v) in map {
                out.push_str(&format!("{inner}<key>{}</key>\n", xml::escape(k)));
                out.push_str(&plist_json(v, &inner));
            }
            out.push_str(&format!("{indent}</dict>\n"));
            out
        },
    }
}
