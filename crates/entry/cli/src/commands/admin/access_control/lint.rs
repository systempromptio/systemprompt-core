//! `admin access-control lint` command: reports unknown and unreachable rule
//! entities.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use anyhow::{Result, anyhow};
use systemprompt_runtime::AppContext;
use systemprompt_security::authz::repository::AccessControlRepository;
use systemprompt_security::authz::types::EntityKind;

use super::LintArgs;
use crate::CliConfig;

const ALL_KINDS: &[EntityKind] = &[
    EntityKind::GatewayRoute,
    EntityKind::McpServer,
    EntityKind::Plugin,
    EntityKind::Agent,
    EntityKind::Marketplace,
    EntityKind::Skill,
    EntityKind::Hook,
    EntityKind::SlackWorkspace,
    EntityKind::SlackChannel,
    EntityKind::TeamsTenant,
    EntityKind::TeamsConversation,
];

// Why: * **Unknown entities** — rows in `access_control_rules` whose
// `(entity_type, entity_id)` has no matching catalog row. The FK added in
// migration 007 makes this impossible going forward, but the check is cheap and
// catches manual SQL fixes that bypass the schema.
// * **Unreachable entities** — catalog rows with `default_included = false`
// and zero matching grants. The entity is registered but no one can reach
// it.
pub(super) async fn run(_args: LintArgs, _config: &CliConfig) -> Result<(String, bool)> {
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
            .list_role_rules_for_export()
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
