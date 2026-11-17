//! The objects a member's own SQL creates and has not dropped since: tables,
//! the indexes and constraint indexes on them, schemas, and the other named
//! objects (types, routines, sequences, views) a `DROP` can name. The
//! member's migrations are read in version order, then its declarative
//! schema, so an object a later migration of the member drops is no longer
//! its own unless the declarative schema creates it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use pg_query::NodeEnum;
use pg_query::protobuf::{ConstrType, Node};

use super::statements::{object_name, qualified, relation_name, top_level};

#[derive(Debug, Clone, Default)]
pub(super) struct Created {
    pub(super) tables: BTreeSet<String>,
    pub(super) indexes: BTreeSet<String>,
    pub(super) objects: BTreeSet<String>,
    pub(super) schemas: BTreeSet<String>,
}

impl Created {
    pub(super) fn read(&mut self, sql: &str) -> Result<(), pg_query::Error> {
        let parsed = pg_query::parse(sql)?;
        for statement in top_level(&parsed) {
            self.statement(statement);
        }
        Ok(())
    }

    fn statement(&mut self, statement: &NodeEnum) {
        match statement {
            NodeEnum::CreateStmt(create) => {
                let Some(table) = create.relation.as_ref().and_then(relation_name) else {
                    return;
                };
                self.tables.insert(table.clone());
                self.objects.insert(table.clone());
                for element in &create.table_elts {
                    self.element(&table, element);
                }
            },
            NodeEnum::IndexStmt(index) if !index.idxname.is_empty() => {
                let schema = index
                    .relation
                    .as_ref()
                    .map_or("", |relation| relation.schemaname.as_str());
                self.indexes.insert(in_schema(schema, &index.idxname));
            },
            NodeEnum::AlterTableStmt(alter) => {
                let schema = alter
                    .relation
                    .as_ref()
                    .map_or("", |relation| relation.schemaname.as_str());
                for command in &alter.cmds {
                    let Some(NodeEnum::AlterTableCmd(command)) = command.node.as_ref() else {
                        continue;
                    };
                    if let Some(NodeEnum::Constraint(constraint)) =
                        command.def.as_ref().and_then(|def| def.node.as_ref())
                        && indexed(constraint.contype)
                        && !constraint.conname.is_empty()
                    {
                        self.indexes.insert(in_schema(schema, &constraint.conname));
                    }
                }
            },
            NodeEnum::CreateSchemaStmt(schema) => {
                self.schemas.insert(schema.schemaname.clone());
                for element in schema.schema_elts.iter().filter_map(|e| e.node.as_ref()) {
                    self.statement(element);
                }
            },
            NodeEnum::CreateFunctionStmt(function) => self.named(&function.funcname),
            NodeEnum::CreateEnumStmt(created) => self.named(&created.type_name),
            NodeEnum::CreateDomainStmt(created) => self.named(&created.domainname),
            NodeEnum::CreateRangeStmt(created) => self.named(&created.type_name),
            NodeEnum::CompositeTypeStmt(created) => {
                self.objects
                    .extend(created.typevar.as_ref().and_then(relation_name));
            },
            NodeEnum::CreateSeqStmt(created) => {
                self.objects
                    .extend(created.sequence.as_ref().and_then(relation_name));
            },
            NodeEnum::ViewStmt(created) => {
                self.objects
                    .extend(created.view.as_ref().and_then(relation_name));
            },
            NodeEnum::CreateTableAsStmt(created) => {
                self.objects.extend(
                    created
                        .into
                        .as_ref()
                        .and_then(|into| into.rel.as_ref())
                        .and_then(relation_name),
                );
            },
            NodeEnum::DropStmt(drop) => {
                for name in drop.objects.iter().filter_map(object_name) {
                    self.tables.remove(&name);
                    self.objects.remove(&name);
                    self.indexes.remove(&name);
                }
            },
            _ => {},
        }
    }

    fn named(&mut self, name: &[Node]) {
        let parts: Vec<&str> = name
            .iter()
            .filter_map(|item| match item.node.as_ref()? {
                NodeEnum::String(s) => Some(s.sval.as_str()),
                _ => None,
            })
            .collect();
        self.objects.extend(qualified(&parts));
    }

    fn element(&mut self, table: &str, element: &Node) {
        let (schema, bare) = table.split_once('.').unwrap_or(("public", table));
        match element.node.as_ref() {
            Some(NodeEnum::Constraint(constraint)) if indexed(constraint.contype) => {
                let name = if constraint.conname.is_empty() {
                    implicit(bare, None, constraint.contype)
                } else {
                    constraint.conname.clone()
                };
                self.indexes.insert(format!("{schema}.{name}"));
            },
            Some(NodeEnum::ColumnDef(column)) => {
                for constraint in
                    column
                        .constraints
                        .iter()
                        .filter_map(|c| match c.node.as_ref()? {
                            NodeEnum::Constraint(constraint) => Some(constraint),
                            _ => None,
                        })
                {
                    if indexed(constraint.contype) {
                        let name = if constraint.conname.is_empty() {
                            implicit(bare, Some(&column.colname), constraint.contype)
                        } else {
                            constraint.conname.clone()
                        };
                        self.indexes.insert(format!("{schema}.{name}"));
                    }
                }
            },
            _ => {},
        }
    }
}

fn indexed(contype: i32) -> bool {
    matches!(
        ConstrType::try_from(contype),
        Ok(ConstrType::ConstrPrimary | ConstrType::ConstrUnique | ConstrType::ConstrExclusion)
    )
}

fn implicit(table: &str, column: Option<&str>, contype: i32) -> String {
    match (ConstrType::try_from(contype), column) {
        (Ok(ConstrType::ConstrPrimary), _) => format!("{table}_pkey"),
        (_, Some(column)) => format!("{table}_{column}_key"),
        (_, None) => format!("{table}_key"),
    }
}

fn in_schema(schema: &str, name: &str) -> String {
    if schema.is_empty() {
        format!("public.{name}")
    } else {
        format!("{schema}.{name}")
    }
}
