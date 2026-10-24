//! One entity's access as a canonical value, the shape the access-control
//! plane compares, hashes and stores.
//!
//! The file and the database both reduce to [`CanonicalEntity`]: the entity's
//! `default_included` and its non-user rules, each with the justification it
//! carries (a band's own `why`, else the entity's). Two sides are equal
//! exactly when their canonical JSON is, whatever order or band grouping the
//! file used. [`to_decl`] is the inverse: the most common justification
//! becomes the entity's `why` and a band whose rows all carry another one
//! keeps it as its own. A band whose rows carry several different
//! justifications cannot be written back in this dialect and takes the
//! entity's.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::types::{BandSpec, EntityDecl, EntityDefault};
use crate::authz::error::{AuthzError, AuthzResult};
use crate::authz::types::Access;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CanonicalRule {
    pub rule_type: String,
    pub rule_value: String,
    pub access: Access,
    pub why: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalEntity {
    pub default_included: bool,
    pub rules: BTreeSet<CanonicalRule>,
}

impl CanonicalEntity {
    pub fn to_value(&self) -> AuthzResult<String> {
        serde_json::to_string(self).map_err(AuthzError::Canonical)
    }

    pub fn from_value(value: &str) -> AuthzResult<Self> {
        serde_json::from_str(value).map_err(AuthzError::Canonical)
    }
}

#[must_use]
pub fn from_decl(decl: &EntityDecl) -> CanonicalEntity {
    let entity_why = decl.why.trim();
    let mut rules = BTreeSet::new();
    for (access, bands) in [(Access::Allow, &decl.allow), (Access::Deny, &decl.deny)] {
        for (band, spec) in bands {
            let why = spec.why().map_or(entity_why, str::trim);
            for value in spec.values() {
                rules.insert(CanonicalRule {
                    rule_type: band.clone(),
                    rule_value: value.clone(),
                    access,
                    why: why.to_owned(),
                });
            }
        }
    }
    CanonicalEntity {
        default_included: decl.default.included(),
        rules,
    }
}

#[must_use]
pub fn to_decl(entity: &str, canonical: &CanonicalEntity, owner: Option<String>) -> EntityDecl {
    let why = dominant_why(canonical);
    let mut grouped: BTreeMap<(Access, &str), Vec<&CanonicalRule>> = BTreeMap::new();
    for rule in &canonical.rules {
        grouped
            .entry((rule.access, rule.rule_type.as_str()))
            .or_default()
            .push(rule);
    }
    let mut decl = EntityDecl {
        entity: entity.to_owned(),
        default: EntityDefault::from_included(canonical.default_included),
        why: why.clone(),
        owner,
        allow: BTreeMap::new(),
        deny: BTreeMap::new(),
    };
    for ((access, band), rows) in grouped {
        let values: Vec<String> = rows.iter().map(|r| r.rule_value.clone()).collect();
        let whys: BTreeSet<&str> = rows.iter().map(|r| r.why.as_str()).collect();
        let spec = match whys.into_iter().collect::<Vec<_>>().as_slice() {
            [one] if *one != why => BandSpec::Detailed {
                values,
                why: (*one).to_owned(),
            },
            _ => BandSpec::List(values),
        };
        let target = match access {
            Access::Allow => &mut decl.allow,
            Access::Deny => &mut decl.deny,
        };
        target.insert(band.to_owned(), spec);
    }
    decl
}

fn dominant_why(canonical: &CanonicalEntity) -> String {
    let mut tally: BTreeMap<&str, usize> = BTreeMap::new();
    for rule in &canonical.rules {
        *tally.entry(rule.why.as_str()).or_default() += 1;
    }
    tally
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(a.0)))
        .map_or_else(String::new, |(why, _)| why.to_owned())
}
