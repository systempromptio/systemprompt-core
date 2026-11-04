//! [`reindex_facts`]: every derived index of a set of facts, set-based.
//!
//! One read for the set (facts, their kind bodies and their subjects'
//! display names), then one statement per index over the whole set:
//! working-set delete and insert, link delete and insert, subject-link
//! delete and insert (the bridges derived from the links just written),
//! subject ensure, alias upsert, weighted-vector update. Ten statements
//! for a set of any size, against a dozen or more per fact on the
//! per-fact path.
//! Aliases and synonym expansions are derived in Rust from the loaded
//! rows. The alias rows are folded with the live facts in `(updated_at,
//! id)` order and the upsert only moves an alias to a fact at least as
//! recent as its owner, so the owner of a shared alias is the same
//! whichever set or batch wrote it last. A fact in the set that is no
//! longer live contributes nothing and releases what it owned
//! ([`super::release_alias_owners`]), an eleventh statement for such a set.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::PgConnection;
use sqlx::types::Json;
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::{FactStatus, KindDecl, SubjectKey, SynonymsDoc};
use systemprompt_traits::RepositoryError;

use super::aliases::{AliasInput, derive_aliases};
use super::entities::{NamedEntity, entity_subjects};
use super::release::{holds_aliases, release_alias_owners};
use super::rows::{AliasOwner, AliasRows};
use crate::models::{FieldMap, decode_status};
use crate::repository::{
    AliasRepository, FactLinkRepository, SearchIndexRepository, SubjectLinkRepository,
    SubjectRepository, WorkingSetRepository,
};

pub const STATEMENTS_PER_SET: u64 = 11;

/// What one set-based reindex did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReindexReport {
    pub facts: u64,
    pub aliases: u64,
    pub statements: u64,
}

struct Indexed {
    id: BrainFactId,
    kind: String,
    status: FactStatus,
    updated_at: DateTime<Utc>,
    subject: SubjectKey,
    statement: String,
    fields: Json<FieldMap>,
    display_name: Option<String>,
    // JSON: the kind declaration as the plane stored it.
    kind_body: Option<serde_json::Value>,
}

pub async fn reindex_facts(
    conn: &mut PgConnection,
    ids: &[BrainFactId],
    synonyms: &SynonymsDoc,
) -> Result<ReindexReport, RepositoryError> {
    let keys: Vec<String> = ids.iter().map(|id| id.as_str().to_owned()).collect();
    let rows = load_many(&mut *conn, &keys).await?;
    if rows.is_empty() {
        return Ok(ReindexReport::default());
    }
    let keys: Vec<String> = rows.iter().map(|row| row.id.to_string()).collect();
    WorkingSetRepository::refresh_many(&mut *conn, &keys).await?;
    FactLinkRepository::refresh_many(&mut *conn, &keys).await?;
    SubjectLinkRepository::refresh_many(&mut *conn, &keys).await?;
    let subjects: Vec<&SubjectKey> = rows.iter().map(|row| &row.subject).collect();
    SubjectRepository::ensure_many(&mut *conn, &subjects).await?;
    let mut kinds: HashMap<String, Option<KindDecl>> = HashMap::new();
    let entities: Vec<(&Indexed, NamedEntity)> = rows
        .iter()
        .flat_map(|row| {
            let kind = kind_of(&mut kinds, row);
            entity_subjects(kind.as_ref(), &row.fields.0)
                .into_iter()
                .map(move |entity| (row, entity))
                .collect::<Vec<_>>()
        })
        .collect();
    let named: Vec<(SubjectKey, Option<String>)> = entities
        .iter()
        .map(|(_, entity)| (entity.subject.clone(), entity.display_name.clone()))
        .collect();
    SubjectRepository::ensure_many_named(&mut *conn, &named).await?;
    let (contributing, released): (Vec<&Indexed>, Vec<&Indexed>) =
        rows.iter().partition(|row| holds_aliases(row.status));
    let mut by_recency = contributing;
    by_recency.sort_by(|a, b| a.updated_at.cmp(&b.updated_at).then(a.id.cmp(&b.id)));
    let mut entity_aliases: Vec<(&Indexed, NamedEntity)> = entities
        .into_iter()
        .filter(|(row, _)| holds_aliases(row.status))
        .collect();
    entity_aliases.sort_by(|(a, _), (b, _)| a.updated_at.cmp(&b.updated_at).then(a.id.cmp(&b.id)));
    let fact_aliases = by_recency.into_iter().map(|row| {
        let kind = kind_of(&mut kinds, row);
        let derived = derive_aliases(AliasInput {
            subject_id: &row.subject.id,
            display_name: row.display_name.as_deref(),
            kind: kind.as_ref(),
            fields: &row.fields.0,
        });
        let owner = AliasOwner {
            subject: &row.subject,
            fact: &row.id,
            updated_at: row.updated_at,
        };
        (owner, derived)
    });
    let named_aliases = entity_aliases.iter().map(|(row, entity)| {
        let owner = AliasOwner {
            subject: &entity.subject,
            fact: &row.id,
            updated_at: row.updated_at,
        };
        (owner, entity.aliases())
    });
    let aliases = AliasRows::collect(fact_aliases.chain(named_aliases));
    AliasRepository::upsert_many(&mut *conn, &aliases).await?;
    let released: Vec<BrainFactId> = released.into_iter().map(|row| row.id.clone()).collect();
    Box::pin(release_alias_owners(&mut *conn, &released)).await?;
    let expansions: Vec<String> = rows
        .iter()
        .map(|row| synonyms.expansion_text(&row.statement))
        .collect();
    SearchIndexRepository::refresh_many(conn, &keys, &expansions).await?;
    Ok(ReindexReport {
        facts: u64::try_from(rows.len()).unwrap_or(u64::MAX),
        aliases: u64::try_from(aliases.len()).unwrap_or(u64::MAX),
        statements: STATEMENTS_PER_SET,
    })
}

fn kind_of(cache: &mut HashMap<String, Option<KindDecl>>, row: &Indexed) -> Option<KindDecl> {
    let body = row.kind_body.as_ref()?;
    if let Some(found) = cache.get(&row.kind) {
        return found.clone();
    }
    let decoded = match KindDecl::deserialize(body) {
        Ok(kind) => Some(kind),
        Err(error) => {
            tracing::warn!(kind = %row.kind, error = %error, "Brain kind body could not be decoded for reindex");
            None
        },
    };
    cache.insert(row.kind.clone(), decoded.clone());
    decoded
}

async fn load_many(
    conn: &mut PgConnection,
    keys: &[String],
) -> Result<Vec<Indexed>, RepositoryError> {
    if keys.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query!(
        r#"SELECT f.id AS "id!: BrainFactId", f.kind AS "kind!", f.status AS "status!",
                  f.updated_at AS "updated_at!",
                  f.subject_kind AS "subject_kind!", f.subject_id AS "subject_id!",
                  f.statement AS "statement!", f.fields AS "fields!: Json<FieldMap>",
                  s.display_name, k.body AS "kind_body?"
           FROM brain_facts f
           LEFT JOIN brain_subjects s
                  ON s.subject_kind = f.subject_kind AND s.subject_id = f.subject_id
           LEFT JOIN brain_fact_kinds k ON k.id = f.kind
           WHERE f.id = ANY($1::text[])
           ORDER BY array_position($1::text[], f.id)"#,
        keys
    )
    .fetch_all(conn)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(Indexed {
                id: row.id,
                kind: row.kind,
                status: decode_status(&row.status)?,
                updated_at: row.updated_at,
                subject: SubjectKey::new(row.subject_kind, row.subject_id),
                statement: row.statement,
                fields: row.fields,
                display_name: row.display_name,
                kind_body: row.kind_body,
            })
        })
        .collect()
}
