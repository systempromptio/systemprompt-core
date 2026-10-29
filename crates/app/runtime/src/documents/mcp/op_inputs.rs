//! The `ops` array of an edit, read so a whole-document op that names a
//! node is refused instead of silently rewriting everything.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer};
use systemprompt_models::documents::EditOp;

const WHOLE_DOCUMENT: [&str; 2] = ["set_text", "set_html"];

pub(super) fn ops<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<EditOp>, D::Error> {
    let raw = Vec::<serde_json::Value>::deserialize(deserializer)?;
    raw.into_iter()
        .enumerate()
        .map(|(index, value)| {
            let op = value.get("op").and_then(serde_json::Value::as_str);
            if let Some(op) = op.filter(|op| WHOLE_DOCUMENT.contains(op))
                && value.get("node").is_some()
            {
                return Err(D::Error::custom(format!(
                    "ops[{index}]: {op} replaces the whole document and takes no node; \
                     to change one block use replace_text {{node, text}}"
                )));
            }
            serde_json::from_value(value).map_err(D::Error::custom)
        })
        .collect()
}
