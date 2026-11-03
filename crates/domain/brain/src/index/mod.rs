//! The derived indexes a stored fact keeps current: its working-set rows,
//! its subject links, its aliases and its weighted search vector.
//!
//! [`reindex_fact`] is the one entry point, called on the write
//! connection after a fact is inserted, revised, merged or has its fields
//! edited, so every index is written in the same transaction as the fact.
//! It reads the fact, its kind declaration and the synonyms document from
//! the database rather than the pipeline's context, so a console edit and
//! a captured candidate index identically. A fact that no longer exists is
//! a no-op: its rows cascade with the delete.
//!
//! [`refresh_projection`] is the same work for a batch of queued keys: the
//! `brain_fact_index` projection an upgrade migration queues every stored
//! fact into and the consumer sweep drains. It also floors `observed_at` to
//! `created_at`, the one stamp a row that predates the column needs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod aliases;

use serde::Deserialize;
use sqlx::PgConnection;
use sqlx::types::Json;
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::{KindDecl, SubjectKey};
use systemprompt_traits::RepositoryError;

pub use aliases::{AliasInput, derive_aliases};

pub const BRAIN_FACT_INDEX: &str = "brain_fact_index";

use crate::models::FieldMap;
use crate::repository::{
    AliasRepository, FactLinkRepository, SearchIndexRepository, SubjectRepository,
    SynonymsRepository, WorkingSetRepository,
};

struct Indexed {
    subject: SubjectKey,
    statement: String,
    fields: Json<FieldMap>,
    display_name: Option<String>,
    // JSON: the kind declaration as the plane stored it.
    kind_body: Option<serde_json::Value>,
}

pub async fn reindex_fact(
    conn: &mut PgConnection,
    id: &BrainFactId,
) -> Result<(), RepositoryError> {
    let Some(row) = load(&mut *conn, id).await? else {
        return Ok(());
    };
    WorkingSetRepository::refresh(&mut *conn, id).await?;
    FactLinkRepository::refresh(&mut *conn, id).await?;
    let kind = row
        .kind_body
        .as_ref()
        .and_then(|body| KindDecl::deserialize(body).ok());
    let aliases = derive_aliases(AliasInput {
        subject_id: &row.subject.id,
        display_name: row.display_name.as_deref(),
        kind: kind.as_ref(),
        fields: &row.fields.0,
    });
    SubjectRepository::ensure(&mut *conn, &row.subject, row.display_name.as_deref()).await?;
    for alias in &aliases {
        AliasRepository::upsert(&mut *conn, &row.subject, alias).await?;
    }
    let synonyms = SynonymsRepository::load(&mut *conn).await?;
    let expansion = synonyms.expansion_text(&row.statement);
    SearchIndexRepository::refresh(conn, id, &expansion).await
}

pub async fn refresh_projection(
    conn: &mut PgConnection,
    keys: Vec<String>,
) -> Result<u64, RepositoryError> {
    sqlx::query!(
        "UPDATE brain_facts SET observed_at = created_at
         WHERE id = ANY($1) AND observed_at > created_at",
        &keys
    )
    .execute(&mut *conn)
    .await?;
    for key in &keys {
        reindex_fact(&mut *conn, &BrainFactId::new(key.clone())).await?;
    }
    Ok(u64::try_from(keys.len()).unwrap_or(u64::MAX))
}

async fn load(
    conn: &mut PgConnection,
    id: &BrainFactId,
) -> Result<Option<Indexed>, RepositoryError> {
    let row = sqlx::query!(
        r#"SELECT f.subject_kind AS "subject_kind!", f.subject_id AS "subject_id!",
                  f.statement AS "statement!", f.fields AS "fields!: Json<FieldMap>",
                  s.display_name, k.body AS "kind_body?"
           FROM brain_facts f
           LEFT JOIN brain_subjects s
                  ON s.subject_kind = f.subject_kind AND s.subject_id = f.subject_id
           LEFT JOIN brain_fact_kinds k ON k.id = f.kind
           WHERE f.id = $1"#,
        id.as_str()
    )
    .fetch_optional(conn)
    .await?;
    Ok(row.map(|row| Indexed {
        subject: SubjectKey::new(row.subject_kind, row.subject_id),
        statement: row.statement,
        fields: row.fields,
        display_name: row.display_name,
        kind_body: row.kind_body,
    }))
}
