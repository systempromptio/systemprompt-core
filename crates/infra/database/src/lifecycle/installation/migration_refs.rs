//! Refuses, before any database write, a migration that names a trigger or
//! view only a declarative schema file creates.
//!
//! Migrations run before the dependent phase that applies declarative
//! triggers and views, so such a reference works on every database that has
//! already booted on the schema shipping the object and fails the first
//! upgrade from one that has not — which is never the author's workstation.
//! Functions need no check: the routine pre-pass applies every declarative
//! function before any migration runs. `DO $$ … $$` bodies are opaque to the
//! parser, which makes a catalog-guarded reference inside one the sanctioned
//! way for a migration to touch a declarative object.
//!
//! It also refuses any bare `ALTER TABLE … ENABLE/DISABLE TRIGGER <name>`,
//! declarative or not: the named trigger may be retired before the migration
//! runs on a database that skips releases, and the runner already suspends
//! every row trigger on the tables a migration writes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;
use std::sync::Arc;

use pg_query::protobuf::{AlterTableType, ObjectType};
use pg_query::{Context, NodeEnum};
use systemprompt_extension::{Extension, LoaderError};
use tracing::warn;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObjectKind {
    Trigger,
    View,
}

impl ObjectKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Trigger => "trigger",
            Self::View => "view",
        }
    }
}

#[derive(Default)]
struct Objects {
    triggers: HashSet<String>,
    views: HashSet<String>,
}

impl Objects {
    fn contains(&self, kind: ObjectKind, name: &str) -> bool {
        match kind {
            ObjectKind::Trigger => self.triggers.contains(name),
            ObjectKind::View => self.views.contains(name),
        }
    }

    fn absorb(&mut self, other: Self) {
        self.triggers.extend(other.triggers);
        self.views.extend(other.views);
    }
}

struct Reference {
    kind: ObjectKind,
    name: String,
    how: &'static str,
}

struct ParsedMigration {
    extension: String,
    migration: String,
    creates: Objects,
    references: Vec<Reference>,
    toggles: Vec<(String, String)>,
}

pub fn check_migration_references(extensions: &[Arc<dyn Extension>]) -> Result<(), LoaderError> {
    let mut declared = Objects::default();
    let mut migrated = Objects::default();
    let mut migrations = Vec::new();

    for ext in extensions {
        let extension = ext.id().to_owned();
        for schema in ext.schemas() {
            declared.absorb(created_objects(&extension, &schema.sql)?);
        }
        for migration in ext.migrations().into_iter().filter(|m| !m.tombstone) {
            let label = format!("{:03}_{}", migration.version, migration.name);
            let parsed = pg_query::parse(migration.sql).map_err(|e| {
                LoaderError::SchemaInstallationFailed {
                    extension: extension.clone(),
                    message: format!("migration {label}: SQL parse failed: {e}"),
                }
            })?;
            let creates = created_objects_of(&parsed);
            migrated.triggers.extend(creates.triggers.iter().cloned());
            migrated.views.extend(creates.views.iter().cloned());
            migrations.push(ParsedMigration {
                extension: extension.clone(),
                migration: label,
                creates,
                references: references_of(&parsed),
                toggles: named_trigger_toggles(&parsed),
            });
        }
    }

    let mut first: Option<LoaderError> = None;
    for m in &migrations {
        for (table, trigger) in &m.toggles {
            let err = LoaderError::MigrationTogglesTriggerByName {
                extension: m.extension.clone(),
                migration: m.migration.clone(),
                table: table.clone(),
                trigger: trigger.clone(),
            };
            if first.is_some() {
                warn!(error = %err, "Further named trigger toggle in a migration");
            } else {
                first = Some(err);
            }
        }
        for r in &m.references {
            if !declared.contains(r.kind, &r.name)
                || migrated.contains(r.kind, &r.name)
                || m.creates.contains(r.kind, &r.name)
            {
                continue;
            }
            let err = LoaderError::MigrationReferencesDeclarativeObject {
                extension: m.extension.clone(),
                migration: m.migration.clone(),
                kind: r.kind.label().to_owned(),
                object: r.name.clone(),
                how: r.how.to_owned(),
            };
            if first.is_some() {
                warn!(error = %err, "Further declarative-object reference in a migration");
            } else {
                first = Some(err);
            }
        }
    }
    first.map_or(Ok(()), Err)
}

fn created_objects(extension: &str, sql: &str) -> Result<Objects, LoaderError> {
    let parsed = pg_query::parse(sql).map_err(|e| LoaderError::SchemaInstallationFailed {
        extension: extension.to_owned(),
        message: format!("SQL parse failed: {e}"),
    })?;
    Ok(created_objects_of(&parsed))
}

fn created_objects_of(parsed: &pg_query::ParseResult) -> Objects {
    let mut objects = Objects::default();
    for node in top_level(parsed) {
        match node {
            NodeEnum::CreateTrigStmt(t) => {
                objects.triggers.insert(t.trigname.to_lowercase());
            },
            NodeEnum::ViewStmt(v) => {
                if let Some(view) = &v.view {
                    objects.views.insert(view.relname.to_lowercase());
                }
            },
            _ => {},
        }
    }
    objects
}

fn references_of(parsed: &pg_query::ParseResult) -> Vec<Reference> {
    let mut refs = Vec::new();
    for node in top_level(parsed) {
        match node {
            NodeEnum::AlterTableStmt(alter) => {
                for cmd in &alter.cmds {
                    let Some(NodeEnum::AlterTableCmd(cmd)) = &cmd.node else {
                        continue;
                    };
                    if matches!(
                        AlterTableType::try_from(cmd.subtype),
                        Ok(AlterTableType::AtEnableTrig
                            | AlterTableType::AtEnableAlwaysTrig
                            | AlterTableType::AtEnableReplicaTrig
                            | AlterTableType::AtDisableTrig)
                    ) {
                        refs.push(Reference {
                            kind: ObjectKind::Trigger,
                            name: cmd.name.to_lowercase(),
                            how: "ALTER TABLE … ENABLE/DISABLE TRIGGER",
                        });
                    }
                }
            },
            NodeEnum::DropStmt(drop) if !drop.missing_ok => {
                let kind = match ObjectType::try_from(drop.remove_type) {
                    Ok(ObjectType::ObjectTrigger) => ObjectKind::Trigger,
                    Ok(ObjectType::ObjectView) => ObjectKind::View,
                    _ => continue,
                };
                for object in &drop.objects {
                    if let Some(name) = dropped_name(object) {
                        refs.push(Reference {
                            kind,
                            name,
                            how: "DROP without IF EXISTS",
                        });
                    }
                }
            },
            _ => {},
        }
    }
    for (table, context) in &parsed.tables {
        if matches!(context, Context::Select | Context::DML) {
            let name = table.rsplit('.').next().unwrap_or(table).to_lowercase();
            refs.push(Reference {
                kind: ObjectKind::View,
                name,
                how: "a query over the view",
            });
        }
    }
    refs
}

// Why: `DISABLE TRIGGER <name>` fails once the trigger is retired, which on
// a multi-release upgrade can happen before the migration runs (0.58 → 0.60:
// web 102 against a dropped `feedback_capture`). `USER` and `ALL` name no
// trigger and stay allowed; a guarded toggle inside `DO $$ … $$` is opaque
// to the parser and is the sanctioned form.
fn named_trigger_toggles(parsed: &pg_query::ParseResult) -> Vec<(String, String)> {
    let mut toggles = Vec::new();
    for node in top_level(parsed) {
        let NodeEnum::AlterTableStmt(alter) = node else {
            continue;
        };
        let table = alter
            .relation
            .as_ref()
            .map(|r| r.relname.to_lowercase())
            .unwrap_or_default();
        for cmd in &alter.cmds {
            let Some(NodeEnum::AlterTableCmd(cmd)) = &cmd.node else {
                continue;
            };
            if matches!(
                AlterTableType::try_from(cmd.subtype),
                Ok(AlterTableType::AtEnableTrig
                    | AlterTableType::AtEnableAlwaysTrig
                    | AlterTableType::AtEnableReplicaTrig
                    | AlterTableType::AtDisableTrig)
            ) {
                toggles.push((table.clone(), cmd.name.to_lowercase()));
            }
        }
    }
    toggles
}

// Why: a dropped object is a List of name parts (schema, table, trigger) or
// a bare RangeVar for a view; the last part is the object's own name.
fn dropped_name(object: &pg_query::protobuf::Node) -> Option<String> {
    match object.node.as_ref()? {
        NodeEnum::List(list) => list.items.iter().rev().find_map(|item| match &item.node {
            Some(NodeEnum::String(s)) => Some(s.sval.to_lowercase()),
            _ => None,
        }),
        NodeEnum::RangeVar(range) => Some(range.relname.to_lowercase()),
        NodeEnum::String(s) => Some(s.sval.to_lowercase()),
        _ => None,
    }
}

fn top_level(parsed: &pg_query::ParseResult) -> impl Iterator<Item = &NodeEnum> {
    parsed
        .protobuf
        .stmts
        .iter()
        .filter_map(|raw| raw.stmt.as_ref().and_then(|s| s.node.as_ref()))
}
