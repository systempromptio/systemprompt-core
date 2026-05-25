//! `systemprompt admin access-control` — DB → YAML export channel and
//! catalog/lint inspector.
//!
//! Subcommands:
//!
//! * `export-yaml` — read role/department rules from `access_control_rules` and
//!   print them as a YAML snippet matching `AccessControlConfig`. Stdout-only —
//!   never writes a file. The operator pastes the output into the committed
//!   YAML baseline and redeploys. Per-user overrides (`rule_type='user'`) are
//!   operational state and intentionally excluded.
//!
//! * `lint` — read the live `access_control_entities` and
//!   `access_control_rules` tables, then report unknown entities (rules
//!   pointing at no catalog row — only possible if the FK was bypassed
//!   manually, e.g. mid-migration) and unreachable rules (catalog rows with
//!   `default_included=false` and zero grant rows — entity exists but no user
//!   can ever reach it). Exits non-zero on any finding so it can gate CI.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, anyhow};
use clap::{Args, Subcommand};
use systemprompt_database::DbPool;
use systemprompt_runtime::AppContext;
use systemprompt_security::authz::repository::AccessControlRepository;
use systemprompt_security::authz::types::EntityKind;

use crate::CliConfig;
use crate::shared::{CommandResult, render_result};

#[derive(Debug, Clone, Copy, Subcommand)]
pub enum AccessControlCommands {
    #[command(
        about = "Print current role/department rules as a YAML snippet for promotion to the \
                 committed baseline"
    )]
    ExportYaml(ExportYamlArgs),

    #[command(
        about = "Lint the live access-control tables for unknown entities and unreachable rules; \
                 exits non-zero on findings"
    )]
    Lint(LintArgs),
}

#[derive(Debug, Clone, Copy, Args)]
pub struct ExportYamlArgs;

#[derive(Debug, Clone, Copy, Args)]
pub struct LintArgs;

pub async fn execute(cmd: AccessControlCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        AccessControlCommands::ExportYaml(args) => {
            let result = export_yaml(args, config).await?;
            render_result(&result);
            Ok(())
        },
        AccessControlCommands::Lint(args) => {
            let (text, exit_nonzero) = lint(args, config).await?;
            let result = CommandResult::raw_text(text).with_title("Access-control lint");
            render_result(&result);
            if exit_nonzero {
                anyhow::bail!("access-control lint failed; see report above");
            }
            Ok(())
        },
    }
}

async fn export_yaml(_args: ExportYamlArgs, _config: &CliConfig) -> Result<CommandResult<String>> {
    let ctx = AppContext::new().await?;
    let yaml = render_yaml_snapshot(ctx.db_pool()).await?;
    Ok(CommandResult::raw_text(yaml)
        .with_title("Access-control baseline (paste into services/access-control YAML)"))
}

/// Iterate every `EntityKind` and report:
///
/// * **Unknown entities** — rows in `access_control_rules` whose `(entity_type,
///   entity_id)` has no matching catalog row. The FK added in migration 007
///   makes this impossible going forward, but the check is cheap and catches
///   manual SQL fixes that bypass the schema.
/// * **Unreachable entities** — catalog rows with `default_included = false`
///   and zero matching grants. The entity is registered but no one can reach
///   it.
///
/// Returns `(human_report, exit_nonzero)`.
async fn lint(_args: LintArgs, _config: &CliConfig) -> Result<(String, bool)> {
    let ctx = AppContext::new().await?;
    let repo =
        AccessControlRepository::new(ctx.db_pool()).map_err(|e| anyhow!("acquire repo: {e}"))?;

    let mut report = String::new();
    let mut unknown_total = 0usize;
    let mut unreachable_total = 0usize;

    for kind in ALL_KINDS {
        let catalog = repo
            .list_entities(*kind)
            .await
            .map_err(|e| anyhow!("list_entities({kind}): {e}"))?;
        let catalog_ids: BTreeSet<String> = catalog.iter().map(|e| e.id.clone()).collect();

        let rule_rows = repo
            .list_role_department_rules_for_export()
            .await
            .map_err(|e| anyhow!("list rules: {e}"))?;
        let rule_ids: BTreeSet<String> = rule_rows
            .iter()
            .filter(|r| r.entity_type == kind.as_str())
            .map(|r| r.entity_id.clone())
            .collect();

        let unknown: Vec<&String> = rule_ids.difference(&catalog_ids).collect();
        let unreachable: Vec<&str> = catalog
            .iter()
            .filter(|e| !e.default_included && !rule_ids.contains(&e.id))
            .map(|e| e.id.as_str())
            .collect();

        if !unknown.is_empty() || !unreachable.is_empty() {
            report.push_str(&format!("\n[{kind}]\n"));
            for id in &unknown {
                report.push_str(&format!(
                    "  UNKNOWN  {id} (rule rows present, no catalog row)\n"
                ));
            }
            for id in &unreachable {
                report.push_str(&format!(
                    "  UNREACHABLE  {id} (catalog row present, default_included=false, no \
                     grants)\n"
                ));
            }
            unknown_total += unknown.len();
            unreachable_total += unreachable.len();
        }
    }

    if unknown_total == 0 && unreachable_total == 0 {
        Ok(("OK — no access-control findings\n".to_owned(), false))
    } else {
        report.insert_str(
            0,
            &format!("FAIL — {unknown_total} unknown, {unreachable_total} unreachable\n"),
        );
        Ok((report, true))
    }
}

const ALL_KINDS: &[EntityKind] = &[
    EntityKind::GatewayRoute,
    EntityKind::McpServer,
    EntityKind::Plugin,
    EntityKind::Agent,
    EntityKind::Marketplace,
    EntityKind::Skill,
    EntityKind::Hook,
];

async fn render_yaml_snapshot(pool: &DbPool) -> Result<String> {
    let grouped = load_grouped_rules(pool).await?;
    let declared_departments = collect_referenced_departments(&grouped);

    let mut out = String::new();
    out.push_str("# Generated by `systemprompt admin access-control export-yaml`\n");
    out.push_str("# This snapshot reflects this instance's DB at export time.\n");
    out.push_str("# Per-user overrides (rule_type='user') are intentionally omitted.\n\n");
    write_departments(&mut out, &declared_departments);
    out.push('\n');
    write_rules(&mut out, &grouped);
    Ok(out)
}

async fn load_grouped_rules(pool: &DbPool) -> Result<BTreeMap<GroupKey, GroupValue>> {
    let repo = AccessControlRepository::new(pool).map_err(|e| anyhow!("acquire repo: {e}"))?;
    let rows = repo
        .list_role_department_rules_for_export()
        .await
        .map_err(|e| anyhow!("query access_control_rules: {e}"))?;

    let mut grouped: BTreeMap<GroupKey, GroupValue> = BTreeMap::new();
    for row in rows {
        let key = GroupKey {
            entity_type: row.entity_type,
            entity_id: row.entity_id,
            access: row.access,
            justification: row.justification.clone(),
        };
        let entry = grouped.entry(key).or_default();
        match row.rule_type.as_str() {
            "role" => entry.roles.push(row.rule_value),
            "department" => {
                entry.departments.push(row.rule_value.clone());
                entry.referenced_departments.push(row.rule_value);
            },
            _ => {},
        }
    }
    Ok(grouped)
}

fn collect_referenced_departments(grouped: &BTreeMap<GroupKey, GroupValue>) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for v in grouped.values() {
        for d in &v.referenced_departments {
            set.insert(d.clone());
        }
    }
    set
}

fn write_departments(out: &mut String, declared: &BTreeSet<String>) {
    out.push_str("departments:\n");
    if declared.is_empty() {
        out.push_str("  []\n");
        return;
    }
    for name in declared {
        out.push_str(&format!("  - name: {}\n", yaml_scalar(name)));
    }
}

fn write_rules(out: &mut String, grouped: &BTreeMap<GroupKey, GroupValue>) {
    out.push_str("rules:\n");
    if grouped.is_empty() {
        out.push_str("  []\n");
        return;
    }
    for (key, value) in grouped {
        write_rule(out, key, value);
    }
}

fn write_rule(out: &mut String, key: &GroupKey, value: &GroupValue) {
    out.push_str("  - entity_type: ");
    out.push_str(&yaml_scalar(&key.entity_type));
    out.push('\n');
    out.push_str("    entity_id: ");
    out.push_str(&yaml_scalar(&key.entity_id));
    out.push('\n');
    out.push_str("    access: ");
    out.push_str(&yaml_scalar(&key.access));
    out.push('\n');
    write_string_array(out, "    roles", &value.roles);
    write_string_array(out, "    departments", &value.departments);
    if let Some(j) = &key.justification {
        out.push_str("    justification: ");
        out.push_str(&yaml_scalar(j));
        out.push('\n');
    }
}

fn write_string_array(out: &mut String, key: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    out.push_str(key);
    out.push_str(": [");
    out.push_str(
        &items
            .iter()
            .map(|s| yaml_scalar(s))
            .collect::<Vec<_>>()
            .join(", "),
    );
    out.push_str("]\n");
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
struct GroupKey {
    entity_type: String,
    entity_id: String,
    access: String,
    justification: Option<String>,
}

#[derive(Debug, Default)]
struct GroupValue {
    roles: Vec<String>,
    departments: Vec<String>,
    referenced_departments: Vec<String>,
}

fn yaml_scalar(s: &str) -> String {
    let needs_quotes = s.is_empty()
        || s.contains([':', '#', '\n', '"', '\'', '\\'])
        || s.starts_with([
            '-', '?', '!', '&', '*', '[', ']', '{', '}', '|', '>', '%', '@', '`', ' ',
        ])
        || s.trim() != s
        || matches!(
            s.to_lowercase().as_str(),
            "true" | "false" | "yes" | "no" | "on" | "off" | "null" | "~"
        )
        || s.parse::<f64>().is_ok();
    if needs_quotes {
        let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{escaped}\"")
    } else {
        s.to_owned()
    }
}
