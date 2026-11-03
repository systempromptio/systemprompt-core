//! Writing one resolved brain entity on the plane transaction: an upsert of
//! its declaration or, when the resolution carries no value, a delete.
//!
//! An interest's `owner` is a user id or an email; the apply resolves it to
//! the user's id for `owner_user_id` and refuses an owner no user matches.
//! The stored body keeps the declared value, so the declared hash of a
//! portable tree is the same on every instance.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::de::DeserializeOwned;
use sqlx::PgConnection;
use std::sync::Arc;

use systemprompt_extension::prelude::{
    ChangeSource, ConfigPlaneResult, PlaneContext, ResolvedEntity,
};
use systemprompt_identifiers::UserId;
use systemprompt_models::brain::{
    FiltersDoc, ImportanceConfig, InterestDecl, KindDecl, RetrievalDoc, SynonymsDoc, TopicDecl,
    WorkflowProfileDecl,
};
use systemprompt_traits::auth::{AuthResult, UserProvider};

use super::entities::{
    DOC, FILTERS_DOC, IMPORTANCE_DOC, INTEREST, KIND, RETRIEVAL_DOC, SYNONYMS_DOC, TOPIC, WORKFLOW,
};
use super::{backend, invalid};
use crate::models::PlaneSource;
use crate::repository::{
    ConfigDocRepository, InterestRepository, KindRepository, PlaneWrite, TopicRepository,
    WorkflowRepository,
};

pub(super) async fn write_entity(
    ctx: &PlaneContext<'_>,
    conn: &mut PgConnection,
    key: &str,
    resolved: &ResolvedEntity,
) -> ConfigPlaneResult<()> {
    let (kind, id) = key
        .split_once('/')
        .ok_or_else(|| invalid(key, &"unknown entity key"))?;
    let source = plane_source(&resolved.source);
    match resolved.value.as_deref() {
        Some(value) => upsert(ctx, conn, Entity { key, kind, id }, value, source).await,
        None => delete(conn, key, kind, id).await,
    }
}

#[derive(Debug, Clone, Copy)]
struct Entity<'a> {
    key: &'a str,
    kind: &'a str,
    id: &'a str,
}

const fn plane_source(source: &ChangeSource) -> PlaneSource {
    match source {
        ChangeSource::Console(_) => PlaneSource::Console,
        ChangeSource::Code(_) | ChangeSource::Bundle(_) | ChangeSource::Migration => {
            PlaneSource::File
        },
    }
}

async fn upsert(
    ctx: &PlaneContext<'_>,
    conn: &mut PgConnection,
    entity: Entity<'_>,
    value: &str,
    source: PlaneSource,
) -> ConfigPlaneResult<()> {
    let Entity { key, kind, id } = entity;
    let body: serde_json::Value = serde_json::from_str(value).map_err(|e| invalid(key, &e))?;
    let row = PlaneWrite {
        id,
        body: &body,
        source,
    };
    let written = match (kind, id) {
        (KIND, _) => {
            let decl: KindDecl = typed(key, &body)?;
            same_id(key, id, decl.kind.as_str())?;
            KindRepository::upsert(conn, &row).await
        },
        (TOPIC, _) => {
            let decl: TopicDecl = typed(key, &body)?;
            same_id(key, id, decl.id.as_str())?;
            TopicRepository::upsert(conn, &row).await
        },
        (INTEREST, _) => {
            let decl: InterestDecl = typed(key, &body)?;
            same_id(key, id, decl.id.as_str())?;
            let users = ctx.get::<Arc<dyn UserProvider>>().map_err(backend)?;
            let owner = resolve_owner(users.as_ref(), &decl.owner)
                .await
                .map_err(backend)?
                .ok_or_else(|| invalid(key, &format!("owner {} is not a user", decl.owner)))?;
            InterestRepository::upsert(conn, &row, Some(&owner), decl.expires).await
        },
        (WORKFLOW, _) => {
            let decl: WorkflowProfileDecl = typed(key, &body)?;
            same_id(key, id, decl.id.as_str())?;
            WorkflowRepository::upsert(conn, &row).await
        },
        (DOC, FILTERS_DOC) => {
            typed::<FiltersDoc>(key, &body)?;
            ConfigDocRepository::upsert(conn, &row).await
        },
        (DOC, IMPORTANCE_DOC) => {
            typed::<ImportanceConfig>(key, &body)?;
            ConfigDocRepository::upsert(conn, &row).await
        },
        (DOC, RETRIEVAL_DOC) => {
            typed::<RetrievalDoc>(key, &body)?;
            ConfigDocRepository::upsert(conn, &row).await
        },
        (DOC, SYNONYMS_DOC) => {
            typed::<SynonymsDoc>(key, &body)?
                .validate()
                .map_err(|e| invalid(key, &e))?;
            ConfigDocRepository::upsert(conn, &row).await
        },
        _ => return Err(invalid(key, &"unknown entity kind")),
    };
    written.map_err(backend)
}

async fn delete(conn: &mut PgConnection, key: &str, kind: &str, id: &str) -> ConfigPlaneResult<()> {
    let deleted = match (kind, id) {
        (KIND, _) => KindRepository::delete(conn, id).await,
        (TOPIC, _) => TopicRepository::delete(conn, id).await,
        (INTEREST, _) => InterestRepository::delete(conn, id).await,
        (WORKFLOW, _) => WorkflowRepository::delete(conn, id).await,
        (DOC, FILTERS_DOC | IMPORTANCE_DOC | RETRIEVAL_DOC | SYNONYMS_DOC) => {
            ConfigDocRepository::delete(conn, id).await
        },
        _ => return Err(invalid(key, &"unknown entity kind")),
    };
    deleted.map(|_| ()).map_err(backend)
}

// JSON: a plane entity body stays untyped until its kind picks the shape.
fn typed<T: DeserializeOwned>(key: &str, body: &serde_json::Value) -> ConfigPlaneResult<T> {
    T::deserialize(body).map_err(|e| invalid(key, &e))
}

fn same_id(key: &str, id: &str, declared: &str) -> ConfigPlaneResult<()> {
    if id == declared {
        Ok(())
    } else {
        Err(invalid(key, &format!("declares id {declared}")))
    }
}

async fn resolve_owner(users: &dyn UserProvider, owner: &UserId) -> AuthResult<Option<UserId>> {
    if let Some(user) = users.find_by_id(owner).await? {
        return Ok(Some(user.id));
    }
    let declared = owner.as_str().trim();
    if !declared.contains('@') {
        return Ok(None);
    }
    Ok(users
        .find_by_email(&declared.to_lowercase())
        .await?
        .map(|user| user.id))
}
