//! The profile's `extensions` section: the members a profile disables and
//! the custom secrets it grants each member.
//!
//! `extensions.secrets` maps a member id to the secret names and prefixes
//! ([`SecretPattern`]: `acme_api_key`, `acme_*`) the operator grants it. A
//! member reads a custom secret only when it declared the name and a grant
//! covers it; with no grant it reads none. An id that names no composed
//! member is refused at admission, and an empty name, an empty prefix or a
//! bare `*` is refused when the profile parses.
//!
//! [`ProfileSecretReferences`] is every custom-secret name the profile itself
//! references (the object-store credential, a services bundle source's
//! `auth_secret`), found by the structural walk of the services config's
//! references; the grants are not walked.
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
use std::collections::BTreeMap;
use std::sync::Arc;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_yaml::Value;
use systemprompt_identifiers::ExtensionId;
use systemprompt_traits::{SecretPattern, SecretReferences};

use super::Profile;
use crate::env::{INTERPOLATED_TAG, PROVENANCE_KEYWORD, UNRESOLVED_TAG};
use crate::services::secret_refs::{names_reference, referenced_secret_names};

const EXTENSIONS_KEY: &str = "extensions";

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExtensionsConfig {
    #[serde(default, deserialize_with = "disabled_list")]
    #[schemars(extend("x-interpolation-provenance" = true))]
    pub disabled: Vec<DisabledExtension>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub secrets: BTreeMap<ExtensionId, Vec<SecretPattern>>,
}

impl ExtensionsConfig {
    pub fn is_disabled(&self, extension_id: &ExtensionId) -> bool {
        self.disabled.iter().any(|entry| entry.id() == extension_id)
    }

    #[must_use]
    pub fn secret_grants(&self, extension_id: &ExtensionId) -> &[SecretPattern] {
        self.secrets.get(extension_id).map_or(&[], Vec::as_slice)
    }
}

/// The custom-secret names the profile references, outside its grants.
#[derive(Debug, Clone)]
pub struct ProfileSecretReferences {
    names: Option<Arc<[String]>>,
}

impl ProfileSecretReferences {
    #[must_use]
    pub fn of(profile: &Profile) -> Self {
        Self {
            names: referenced_secret_names(profile, &[EXTENSIONS_KEY]).map(Arc::from),
        }
    }
}

impl SecretReferences for ProfileSecretReferences {
    fn references(&self, name: &str) -> bool {
        names_reference(self.names.as_deref(), name)
    }
}

/// One entry of `extensions.disabled`: the id and the variables it was
/// interpolated from (empty for an id written literally).
#[derive(Clone, PartialEq, Eq)]
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

impl std::fmt::Debug for DisabledExtension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_struct("DisabledExtension");
        if self.variables.is_empty() {
            debug.field("id", &self.id);
        } else {
            debug.field("variables", &self.variables);
        }
        debug.finish()
    }
}

#[derive(Debug, thiserror::Error)]
#[error("the value of {variables} is not an extension id: {source}")]
struct NotAnExtensionId {
    variables: String,
    #[source]
    source: systemprompt_identifiers::error::IdValidationError,
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
                    .map_err(|source| {
                        E::custom(NotAnExtensionId {
                            variables: named,
                            source,
                        })
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
