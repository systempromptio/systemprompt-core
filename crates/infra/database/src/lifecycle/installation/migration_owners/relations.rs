//! The relations a statement names besides the one it changes: the parents
//! a `CREATE TABLE ... INHERITS` or `PARTITION OF` attaches to, the parent
//! of `ALTER TABLE ... INHERIT | NO INHERIT`, the partition of `ALTER TABLE
//! ... ATTACH | DETACH PARTITION`, and the statements a `CREATE RULE` runs.
//! Each is a mutation of the named relation: a child's rows join its parent's
//! reads, a partition takes the parent's constraint, and a rule's actions run
//! as the rule's statements. A relation a statement only reads or constrains
//! against (`LIKE ... INCLUDING`, foreign-key `REFERENCES`, a view's or
//! query's `FROM`) is not changed by it and is not judged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use pg_query::NodeEnum;
use pg_query::protobuf::{AlterTableType, Node};

use super::kinds::TableMutation;
use super::statements::{Effect, effects_of, is_table, relation_name};

pub(super) fn secondary_effects(statement: &NodeEnum) -> Vec<Effect> {
    match statement {
        NodeEnum::CreateStmt(create) => {
            let kind = if create.partbound.is_some() {
                TableMutation::Partition
            } else {
                TableMutation::Inherit
            };
            create
                .inh_relations
                .iter()
                .filter_map(range_var)
                .map(|parent| Effect::Table(kind, parent))
                .collect()
        },
        NodeEnum::AlterTableStmt(alter) if is_table(alter.objtype) => {
            alter.cmds.iter().filter_map(command_relation).collect()
        },
        NodeEnum::RuleStmt(rule) => rule
            .actions
            .iter()
            .filter_map(|action| action.node.as_ref())
            .flat_map(effects_of)
            .collect(),
        _ => Vec::new(),
    }
}

fn range_var(node: &Node) -> Option<String> {
    match node.node.as_ref()? {
        NodeEnum::RangeVar(relation) => relation_name(relation),
        _ => None,
    }
}

fn command_relation(command: &Node) -> Option<Effect> {
    let NodeEnum::AlterTableCmd(command) = command.node.as_ref()? else {
        return None;
    };
    let def = command.def.as_deref()?;
    match AlterTableType::try_from(command.subtype).ok()? {
        AlterTableType::AtAddInherit | AlterTableType::AtDropInherit => {
            range_var(def).map(|parent| Effect::Table(TableMutation::Inherit, parent))
        },
        AlterTableType::AtAttachPartition
        | AlterTableType::AtDetachPartition
        | AlterTableType::AtDetachPartitionFinalize => match def.node.as_ref()? {
            NodeEnum::PartitionCmd(partition) => partition
                .name
                .as_ref()
                .and_then(relation_name)
                .map(|table| Effect::Table(TableMutation::Partition, table)),
            _ => None,
        },
        _ => None,
    }
}
