//! The profile's `extensions` section: the members a profile disables.
//!
//! Each disabled entry keeps the variables it was interpolated from, so a
//! refusal of an entry (an unknown id, an id that does not validate, a value
//! where a list belongs) names "the value of ${NAME}" and never prints the
//! resolved value. The provenance comes from the interpolation walk: the
//! schema marks the list and its items with [`PROVENANCE_KEYWORD`], and a
//! resolved value arrives wrapped in an [`INTERPOLATED_TAG`] marker.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_yaml::Value;
use systemprompt_identifiers::ExtensionId;

use crate::env::{INTERPOLATED_TAG, PROVENANCE_KEYWORD, UNRESOLVED_TAG};

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExtensionsConfig {
    #[serde(default, deserialize_with = "disabled_list")]
    #[schemars(extend("x-interpolation-provenance" = true))]
    pub disabled: Vec<DisabledExtension>,
}

impl ExtensionsConfig {
    pub fn is_disabled(&self, extension_id: &ExtensionId) -> bool {
        self.disabled.iter().any(|entry| entry.id() == extension_id)
    }
}

/// One entry of `extensions.disabled`: the id and the variables it was
/// interpolated from (empty for an id written literally).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisabledExtension {
    id: ExtensionId,
    variables: Vec<String>,
}

impl DisabledExtension {
    #[must_use]
    pub const fn literal(id: ExtensionId) -> Self {
        Self {
            id,
            variables: Vec::new(),
        }
    }

    #[must_use]
    pub const fn interpolated(id: ExtensionId, variables: Vec<String>) -> Self {
        Self { id, variables }
    }

    #[must_use]
    pub const fn id(&self) -> &ExtensionId {
        &self.id
    }

    #[must_use]
    pub fn variables(&self) -> &[String] {
        &self.variables
    }
}

impl std::fmt::Display for DisabledExtension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.variables.is_empty() {
            return f.write_str(self.id.as_str());
        }
        f.write_str("the value of ")?;
        Variables(&self.variables).fmt(f)
    }
}

struct Variables<'a>(&'a [String]);

impl std::fmt::Display for Variables<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (index, name) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            write!(f, "${{{name}}}")?;
        }
        Ok(())
    }
}

impl Serialize for DisabledExtension {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.id.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DisabledExtension {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        entry(Value::deserialize(deserializer)?)
    }
}

impl schemars::JsonSchema for DisabledExtension {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("DisabledExtension")
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            (PROVENANCE_KEYWORD): true,
        })
    }
}

fn entry<E: serde::de::Error>(value: Value) -> Result<DisabledExtension, E> {
    match value {
        Value::String(id) => ExtensionId::try_new(id)
            .map(DisabledExtension::literal)
            .map_err(E::custom),
        Value::Tagged(tagged) if tagged.tag == INTERPOLATED_TAG => {
            let (variables, value) = marker(tagged.value);
            let named = Variables(&variables).to_string();
            match value {
                Value::String(id) => ExtensionId::try_new(id)
                    .map(|id| DisabledExtension::interpolated(id, variables))
                    .map_err(|error| {
                        E::custom(format!(
                            "the value of {named} is not an extension id: {error}"
                        ))
                    }),
                _ => Err(E::custom(format!(
                    "the value of {named} is not an extension id"
                ))),
            }
        },
        Value::Tagged(tagged) if tagged.tag == UNRESOLVED_TAG => Err(E::custom(
            "an extensions.disabled entry references a variable that is not set",
        )),
        _ => Err(E::custom(
            "an extensions.disabled entry is not an extension id",
        )),
    }
}

fn marker(value: Value) -> (Vec<String>, Value) {
    let Value::Mapping(mut map) = value else {
        return (Vec::new(), Value::Null);
    };
    let variables = match map.remove("variables") {
        Some(Value::Sequence(items)) => items
            .into_iter()
            .filter_map(|item| match item {
                Value::String(name) => Some(name),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    (variables, map.remove("value").unwrap_or(Value::Null))
}

fn disabled_list<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<DisabledExtension>, D::Error> {
    match Value::deserialize(deserializer)? {
        Value::Null => Ok(Vec::new()),
        Value::Sequence(items) => items.into_iter().map(entry).collect(),
        Value::Tagged(tagged) if tagged.tag == INTERPOLATED_TAG => {
            let (variables, _) = marker(tagged.value);
            Err(D::Error::custom(format!(
                "extensions.disabled holds the value of {}, not a list of extension ids",
                Variables(&variables)
            )))
        },
        _ => Err(D::Error::custom(
            "extensions.disabled is not a list of extension ids",
        )),
    }
}
