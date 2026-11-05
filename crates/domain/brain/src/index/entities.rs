//! The entity subjects a fact names.
//!
//! Every entity field whose value is `<ref>:<id>` is a subject of kind
//! `<ref>`, ensured by the reindex with the display name its declared
//! `display` sibling carries and aliased by its id (as written and with
//! `-`/`_` as spaces) and that name. A person a team fact names then
//! matches like a client, and the bridge the fact declares has a subject
//! to anchor on.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::brain::{FieldType, KindDecl, SubjectKey};

use crate::models::FieldMap;

/// One entity subject a fact names, with the display name it carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedEntity {
    pub subject: SubjectKey,
    pub display_name: Option<String>,
}

impl NamedEntity {
    #[must_use]
    pub fn aliases(&self) -> Vec<String> {
        let mut out = vec![self.subject.id.clone()];
        let spaced = self.subject.id.replace(['-', '_'], " ");
        if spaced != self.subject.id {
            out.push(spaced);
        }
        if let Some(name) = &self.display_name
            && !out.iter().any(|seen| seen == name)
        {
            out.push(name.clone());
        }
        out
    }
}

#[must_use]
pub fn entity_subjects(kind: Option<&KindDecl>, fields: &FieldMap) -> Vec<NamedEntity> {
    let Some(kind) = kind else {
        return Vec::new();
    };
    kind.fields
        .iter()
        .filter(|(_, decl)| decl.field_type == FieldType::Entity)
        .filter_map(|(name, decl)| {
            let expected = decl.entity_ref.as_deref()?;
            let value = fields.get(name)?.as_str()?;
            let (found, id) = value.split_once(':')?;
            if found != expected || id.trim().is_empty() {
                return None;
            }
            let display_name = decl
                .display
                .as_deref()
                .and_then(|sibling| fields.get(sibling))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned);
            Some(NamedEntity {
                subject: SubjectKey::new(expected, id.trim()),
                display_name,
            })
        })
        .collect()
}
