//! The renames, drops, grants and copies a migration statement makes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use pg_query::NodeEnum;
use pg_query::protobuf::{DropBehavior, GrantTargetType, ObjectType};

use super::kinds::{HiddenSql, TableMutation};
use super::statements::{
    Effect, is_table, object_name, object_on_table, object_type, on, relation_name, strings,
    trigger_key, trigger_of,
};

pub(super) fn renamed(rename: &pg_query::protobuf::RenameStmt) -> Option<Vec<Effect>> {
    Some(match object_type(rename.rename_type) {
        Some(ObjectType::ObjectTrigger) => {
            let mut effects = on(TableMutation::Rename, rename.relation.as_ref());
            effects.extend(
                rename
                    .relation
                    .as_ref()
                    .and_then(relation_name)
                    .map(|table| {
                        Effect::Object(TableMutation::Rename, trigger_key(&rename.subname, &table))
                    }),
            );
            effects
        },
        Some(
            ObjectType::ObjectTable
            | ObjectType::ObjectForeignTable
            | ObjectType::ObjectColumn
            | ObjectType::ObjectTabconstraint
            | ObjectType::ObjectPolicy
            | ObjectType::ObjectRule,
        ) => on(TableMutation::Rename, rename.relation.as_ref()),
        Some(ObjectType::ObjectSchema) => vec![Effect::Schema(
            TableMutation::RenameSchema,
            Some(rename.subname.clone()),
        )],
        Some(ObjectType::ObjectIndex) => rename
            .relation
            .as_ref()
            .and_then(relation_name)
            .map(|index| Effect::Object(TableMutation::AlterIndex, index))
            .into_iter()
            .collect(),
        Some(
            ObjectType::ObjectView
            | ObjectType::ObjectMatview
            | ObjectType::ObjectSequence
            | ObjectType::ObjectAttribute,
        ) => rename
            .relation
            .as_ref()
            .and_then(relation_name)
            .map(|object| Effect::Object(TableMutation::AlterObject, object))
            .into_iter()
            .collect(),
        Some(
            ObjectType::ObjectFunction
            | ObjectType::ObjectProcedure
            | ObjectType::ObjectRoutine
            | ObjectType::ObjectAggregate
            | ObjectType::ObjectType
            | ObjectType::ObjectDomain
            | ObjectType::ObjectDomconstraint,
        ) => rename
            .object
            .as_deref()
            .and_then(object_name)
            .map(|object| Effect::Object(TableMutation::AlterObject, object))
            .into_iter()
            .collect(),
        _ => return None,
    })
}

pub(super) fn dropped(drop: &pg_query::protobuf::DropStmt) -> Vec<Effect> {
    let cascade = DropBehavior::try_from(drop.behavior) == Ok(DropBehavior::DropCascade);
    let remove = object_type(drop.remove_type);
    if is_table(drop.remove_type) {
        return drop
            .objects
            .iter()
            .filter_map(object_name)
            .map(|table| Effect::Table(TableMutation::Drop, table))
            .collect();
    }
    let on_table = match remove {
        Some(ObjectType::ObjectTrigger) => Some(TableMutation::DropTrigger),
        Some(ObjectType::ObjectPolicy) => Some(TableMutation::DropPolicy),
        Some(ObjectType::ObjectRule) => Some(TableMutation::DropRule),
        _ => None,
    };
    let mut effects = Vec::new();
    for object in &drop.objects {
        if let Some(kind) = on_table {
            effects.extend(object_on_table(object).map(|table| Effect::Table(kind, table)));
            if kind == TableMutation::DropTrigger {
                effects.extend(trigger_of(object).map(|key| Effect::Object(kind, key)));
            }
            continue;
        }
        if remove == Some(ObjectType::ObjectSchema) {
            let schema = match object.node.as_ref() {
                Some(NodeEnum::String(s)) => Some(s.sval.clone()),
                _ => None,
            };
            effects.push(Effect::Schema(TableMutation::DropSchema, schema));
            continue;
        }
        let Some(name) = object_name(object) else {
            continue;
        };
        let kind = if remove == Some(ObjectType::ObjectIndex) {
            TableMutation::DropIndex
        } else {
            TableMutation::DropObject
        };
        effects.push(Effect::Object(kind, name.clone()));
        if cascade {
            effects.push(Effect::Cascade(name));
        }
    }
    effects
}

pub(super) fn granted(grant: &pg_query::protobuf::GrantStmt) -> Option<Vec<Effect>> {
    let kind = if grant.is_grant {
        TableMutation::Grant
    } else {
        TableMutation::Revoke
    };
    if GrantTargetType::try_from(grant.targtype) == Ok(GrantTargetType::AclTargetAllInSchema) {
        return Some(
            strings(&grant.objects)
                .into_iter()
                .map(|schema| Effect::Schema(TableMutation::GrantInSchema, Some(schema.to_owned())))
                .collect(),
        );
    }
    if GrantTargetType::try_from(grant.targtype) != Ok(GrantTargetType::AclTargetObject) {
        return None;
    }
    let relations = || {
        grant
            .objects
            .iter()
            .filter_map(|node| match node.node.as_ref()? {
                NodeEnum::RangeVar(relation) => relation_name(relation),
                _ => None,
            })
    };
    Some(match object_type(grant.objtype)? {
        ObjectType::ObjectTable | ObjectType::ObjectForeignTable => relations()
            .map(|table| Effect::Table(kind, table))
            .collect(),
        ObjectType::ObjectSequence => relations()
            .map(|sequence| Effect::Object(kind, sequence))
            .collect(),
        ObjectType::ObjectFunction
        | ObjectType::ObjectProcedure
        | ObjectType::ObjectRoutine
        | ObjectType::ObjectType
        | ObjectType::ObjectDomain => grant
            .objects
            .iter()
            .filter_map(object_name)
            .map(|object| Effect::Object(kind, object))
            .collect(),
        ObjectType::ObjectSchema => strings(&grant.objects)
            .into_iter()
            .map(|schema| Effect::Schema(TableMutation::GrantOnSchema, Some(schema.to_owned())))
            .collect(),
        _ => return None,
    })
}

pub(super) fn copied(copy: &pg_query::protobuf::CopyStmt) -> Vec<Effect> {
    let mut effects = Vec::new();
    if copy.is_program || !copy.filename.is_empty() {
        effects.push(Effect::Hidden(HiddenSql::CopyProgram));
    }
    if copy.is_from {
        effects.extend(
            copy.relation
                .as_ref()
                .and_then(relation_name)
                .map(|table| Effect::Write(TableMutation::Copy, table, None)),
        );
    }
    effects
}
