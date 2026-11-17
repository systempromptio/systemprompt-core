//! The judgement of a statement that names an object or a schema: a create
//! of another member's object, any other statement on a named object, and a
//! schema-level statement.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::claims::Ownership;
use super::judge::Judged;
use super::object_claims::Prefer;
use super::objects::Created;
use super::{MigrationOwnershipError, TableMutation};

pub(super) fn judge_schema(
    ownership: &Ownership,
    member: &Judged<'_>,
    statement: TableMutation,
    schema: Option<String>,
) -> Option<MigrationOwnershipError> {
    let refused = schema
        .as_deref()
        .is_none_or(|schema| schema == "public" || ownership.held_by_another(schema, member.id));
    if !refused {
        member.unjudged(&statement, schema.as_deref().unwrap_or_default());
        return None;
    }
    Some(MigrationOwnershipError::SchemaLevel {
        member: member.id.clone(),
        migration: member.migration.to_owned(),
        statement,
        schema: schema.unwrap_or_else(|| String::from("(every schema)")),
    })
}

pub(super) fn foreign_object_creates(
    ownership: &Ownership,
    member: &Judged<'_>,
    statement: &pg_query::NodeEnum,
) -> Vec<MigrationOwnershipError> {
    let created = Created::of_statement(statement);
    let mut refused = Vec::new();
    for (kind, object, owner) in
        ownership.foreign_object_creates(member.id, member.standing, &created)
    {
        if member.install() {
            refused.push(MigrationOwnershipError::ForeignObject {
                member: member.id.clone(),
                migration: member.migration.to_owned(),
                statement: kind,
                object,
                owner,
            });
        } else {
            member.unjudged(&kind, &object);
        }
    }
    refused
}

pub(super) fn judge_named(
    ownership: &Ownership,
    member: &Judged<'_>,
    statement: TableMutation,
    object: String,
    prefer: Prefer,
) -> Option<MigrationOwnershipError> {
    let owner = ownership.resolve(&object, prefer);
    if owner.is_some_and(|owner| member.owns(owner)) {
        return None;
    }
    match owner {
        Some(owner) if member.install() => Some(MigrationOwnershipError::ForeignObject {
            member: member.id.clone(),
            migration: member.migration.to_owned(),
            statement,
            object,
            owner: owner.id.clone(),
        }),
        Some(_) => {
            member.unjudged(&statement, &object);
            None
        },
        None => {
            if prefer == Prefer::Index {
                member.unjudged(&statement, &object);
            }
            None
        },
    }
}
