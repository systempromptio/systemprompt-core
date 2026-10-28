//! The per-entity three-way merge of an explicit apply (credential.md §3.1).
//!
//! Each entity has three hashes: the base (what the last apply wrote, from
//! `config_plane_base`), the file (the declaration now) and the database
//! (the row now). `None` on any side means the entity is absent there. The
//! verdict is decided by which sides moved away from the base:
//!
//! | file moved | database moved | verdict |
//! |---|---|---|
//! | no | no | [`Verdict::Unchanged`] |
//! | yes | no | [`Verdict::Apply`] |
//! | no | yes | [`Verdict::Drift`] (the console edit is kept) |
//! | yes | yes | [`Verdict::Conflict`], unless both moved to the same value ([`Verdict::Converged`]) |
//!
//! [`merge_plane`] merges every entity named on any of the three sides;
//! `base` maps entity → the hash the last apply recorded, and `adopt` is
//! true when the plane has never been applied. An adopted entity records its
//! adopted base even though nothing is written for it.
//!
//! A plane that has never been applied has no base. Its first apply adopts the
//! database: every entity present on both sides takes the file's hash as its
//! base, so a value that differs is kept and reported as drift instead of
//! being overwritten, while a declared entity the database lacks is still
//! inserted and a database-only row is still kept.
//!
//! A `file`-authority plane merges with `file_wins`: there is no third side
//! to weigh, so every entity whose file and database sides differ is applied
//! (a database-only row is deleted) and nothing is kept as drift or conflict.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use systemprompt_provider_contracts::{DeclaredEntities, PlaneSnapshot, entity_hash};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Unchanged,
    Apply,
    Converged,
    Drift,
    Conflict,
}

#[must_use]
pub fn verdict(base: Option<&str>, file: Option<&str>, db: Option<&str>) -> Verdict {
    if file == db {
        return if base == file {
            Verdict::Unchanged
        } else {
            Verdict::Converged
        };
    }
    match (file == base, db == base) {
        (true, _) => Verdict::Drift,
        (false, true) => Verdict::Apply,
        (false, false) => Verdict::Conflict,
    }
}

#[must_use]
pub fn file_wins_verdict(base: Option<&str>, file: Option<&str>, db: Option<&str>) -> Verdict {
    match (file == db, base == file) {
        (false, _) => Verdict::Apply,
        (true, true) => Verdict::Unchanged,
        (true, false) => Verdict::Converged,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EntityMerge {
    pub entity: String,
    pub verdict: Verdict,
    pub base_hash: Option<String>,
    pub file_hash: Option<String>,
    pub db_hash: Option<String>,
    pub adopted: bool,
}

#[must_use]
pub fn merge_plane(
    declared: &DeclaredEntities,
    current: &PlaneSnapshot,
    base: &BTreeMap<String, String>,
    adopt: bool,
    file_wins: bool,
) -> Vec<EntityMerge> {
    let entities: BTreeSet<&String> = declared
        .keys()
        .chain(current.keys())
        .chain(base.keys())
        .collect();
    entities
        .into_iter()
        .map(|entity| {
            let file_hash = declared.get(entity).map(|d| d.hash.clone());
            let db_hash = current.get(entity).map(|value| entity_hash(value));
            let adopted = adopt && !file_wins && file_hash.is_some() && db_hash.is_some();
            let base_hash = if adopted {
                file_hash.clone()
            } else {
                base.get(entity).cloned()
            };
            let decide = if file_wins {
                file_wins_verdict
            } else {
                verdict
            };
            EntityMerge {
                entity: entity.clone(),
                verdict: decide(
                    base_hash.as_deref(),
                    file_hash.as_deref(),
                    db_hash.as_deref(),
                ),
                base_hash,
                file_hash,
                db_hash,
                adopted,
            }
        })
        .collect()
}
