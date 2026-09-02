//! The on-disk shape of a managed policy, independent of the OS store that
//! holds it: a flat map of named values.
//!
//! Windows stores every value as a `REG_SZ` string, so only [`Str`] is ever
//! read or written there; macOS Managed Preferences carry typed plist values.
//!
//! [`Str`]: PolicyDocumentValue::Str
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Which policy scope a store operation addresses. Windows: `HKLM` versus
/// `HKCU`; macOS: the system plist versus the per-user plist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolicyHive {
    Machine,
    User,
}

impl PolicyHive {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Machine => "HKLM",
            Self::User => "HKCU",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum PolicyDocumentValue {
    Str(String),
    Bool(bool),
    StrList(Vec<String>),
    Dicts(Vec<BTreeMap<String, Self>>),
}

impl PolicyDocumentValue {
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }
}

pub type PolicyDocument = BTreeMap<String, PolicyDocumentValue>;

impl PolicyDocumentValue {
    // Why: `plutil -convert json` is how a plist is read back; the JSON shape
    // has to round-trip to the same value the renderer wrote.
    #[must_use]
    pub fn from_json(v: &serde_json::Value) -> Option<Self> {
        match v {
            serde_json::Value::String(s) => Some(Self::Str(s.clone())),
            serde_json::Value::Bool(b) => Some(Self::Bool(*b)),
            serde_json::Value::Array(items) => {
                if items.iter().all(serde_json::Value::is_string) {
                    return Some(Self::StrList(
                        items
                            .iter()
                            .filter_map(serde_json::Value::as_str)
                            .map(str::to_owned)
                            .collect(),
                    ));
                }
                let mut dicts = Vec::with_capacity(items.len());
                for item in items {
                    let obj = item.as_object()?;
                    let mut dict = BTreeMap::new();
                    for (k, v) in obj {
                        dict.insert(k.clone(), Self::from_json(v)?);
                    }
                    dicts.push(dict);
                }
                Some(Self::Dicts(dicts))
            },
            serde_json::Value::Object(obj) => {
                let mut dict = BTreeMap::new();
                for (k, v) in obj {
                    dict.insert(k.clone(), Self::from_json(v)?);
                }
                Some(Self::Dicts(vec![dict]))
            },
            serde_json::Value::Null | serde_json::Value::Number(_) => None,
        }
    }
}
