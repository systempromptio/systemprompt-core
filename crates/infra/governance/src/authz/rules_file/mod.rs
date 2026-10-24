//! `services/access-control/rules.yaml`: core's one access-control dialect.
//!
//! [`read_rules_doc`] reads and validates the file (absent is `None`: an
//! instance may ship no rules), [`declared_entities`] reduces it to one
//! canonical value per entity, and [`render_rules_doc`] writes canonical
//! values back as the file. The access-control config plane is built on these
//! three; nothing else parses the file.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod canonical;
mod types;
mod validate;

use std::collections::BTreeMap;
use std::path::Path;

pub use canonical::{CanonicalEntity, CanonicalRule, from_decl, to_decl};
pub use types::{
    BandSpec, EntityDecl, EntityDefault, EntityTarget, GLOB_KINDS, OWNER_BUNDLE_PREFIX,
    RETIRED_ROLES_FILE, RULES_FILE, RulesDoc,
};

pub use validate::{RulesProblem, RulesProblems};

use crate::authz::error::{AuthzError, AuthzResult};

const EXPORT_HEADER: &str = "\
# services/access-control/rules.yaml, rendered from the database.
# Commit it to make the database state the declared state. Per-person (user
# band) overrides are runtime state and are never written here.
";

pub fn parse_rules_doc(text: &str) -> AuthzResult<RulesDoc> {
    if text.trim().is_empty() {
        return Ok(RulesDoc::default());
    }
    let doc: RulesDoc = serde_yaml::from_str(text).map_err(|source| AuthzError::ConfigParse {
        path: RULES_FILE.to_owned(),
        source,
    })?;
    doc.validate()?;
    Ok(doc)
}

pub fn read_rules_doc(services_root: &Path) -> AuthzResult<Option<RulesDoc>> {
    let path = services_root.join(RULES_FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) => parse_rules_doc(&text).map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(AuthzError::File {
            action: "read",
            path: path.display().to_string(),
            source,
        }),
    }
}

/// One declared entity: its canonical value and the bundle that owns it,
/// keyed by `<kind>/<id>` as the file wrote it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredAccess {
    pub canonical: CanonicalEntity,
    pub owner_bundle: Option<String>,
}

pub fn declared_entities(doc: &RulesDoc) -> AuthzResult<BTreeMap<String, DeclaredAccess>> {
    doc.entities
        .iter()
        .map(|decl| {
            let target: EntityTarget = decl.entity.parse()?;
            Ok((
                target.to_string(),
                DeclaredAccess {
                    canonical: from_decl(decl),
                    owner_bundle: decl.owner_bundle().map(str::to_owned),
                },
            ))
        })
        .collect()
}

pub fn render_rules_doc(
    entities: &BTreeMap<String, CanonicalEntity>,
    owners: &BTreeMap<String, String>,
) -> AuthzResult<String> {
    let doc = RulesDoc {
        entities: entities
            .iter()
            .map(|(entity, canonical)| {
                let owner = owners
                    .get(entity)
                    .map(|name| format!("{OWNER_BUNDLE_PREFIX}{name}"));
                to_decl(entity, canonical, owner)
            })
            .collect(),
    };
    let body = serde_yaml::to_string(&doc).map_err(|source| AuthzError::ConfigParse {
        path: RULES_FILE.to_owned(),
        source,
    })?;
    Ok(format!("{EXPORT_HEADER}\n{body}"))
}
