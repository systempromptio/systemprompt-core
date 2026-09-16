//! Post-ingest existence check on the subjects a rule names.
//!
//! A rule is `(entity, rule_type, rule_value)`. Nothing in the schema ties
//! `rule_value` to a subject that exists, so `roles: [enginering]` ingests
//! cleanly and grants nobody anything — an inert rule that reads, in the access
//! matrix, exactly like a live one.
//!
//! Only the `role` dimension is checkable, and only through a registered
//! [`RoleDirectory`]: roles live in the users domain, so a role is real when
//! that domain says some user holds it. Group and project dimensions have no
//! core table — they are extension-owned subject dimensions registered
//! through `SubjectAttributeProvider` — so a value in those bands, or any
//! role when no directory is registered, is unverifiable rather than unknown,
//! and no warning is emitted for it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use super::super::error::AuthzResult;
use super::super::subject_directory::RoleDirectory;
use super::super::types::RuleType;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnknownSubject {
    pub rule_type: String,
    pub value: String,
    pub entity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct SubjectMention {
    pub(super) rule_type: String,
    pub(super) value: String,
    pub(super) entity: String,
}

pub(super) async fn find_unknown_subjects(
    directory: Option<&dyn RoleDirectory>,
    mentions: &BTreeSet<SubjectMention>,
) -> AuthzResult<Vec<UnknownSubject>> {
    let Some(directory) = directory else {
        return Ok(Vec::new());
    };
    let roles: Vec<String> = mentions
        .iter()
        .filter(|m| m.rule_type == RuleType::ROLE.to_string())
        .map(|m| m.value.clone())
        .collect();
    if roles.is_empty() {
        return Ok(Vec::new());
    }

    let missing: BTreeSet<String> = directory.unknown_roles(&roles).await?.into_iter().collect();
    let mut out = Vec::new();
    for mention in mentions {
        if mention.rule_type != RuleType::ROLE.to_string() || !missing.contains(&mention.value) {
            continue;
        }
        tracing::warn!(
            rule_type = %mention.rule_type,
            value = %mention.value,
            entity = %mention.entity,
            "authz rule names a subject that does not exist"
        );
        out.push(UnknownSubject {
            rule_type: mention.rule_type.clone(),
            value: mention.value.clone(),
            entity: mention.entity.clone(),
        });
    }
    Ok(out)
}
