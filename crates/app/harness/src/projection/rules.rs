//! The rules as the managed block of the global `AGENTS.md`, byte for byte the
//! block the bridge writes, markers included, so a block it left is replaced
//! too.
//!
//! Only rules that name the host are written (a rule with no `hosts` reaches no
//! host), sorted by id, each as a `## <name>` heading and its trimmed
//! instructions; a rule with empty instructions keeps its heading. Splicing
//! cuts any existing block out, keeps what the user wrote and appends the block
//! after it; a file left with nothing is deleted.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_device::protocol::targets::rule_targets_host;
use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::bridge::manifest::RuleEntry;

use crate::projection::plan::unique_plain;

pub const RULES_BEGIN: &str =
    "<!-- systemprompt:begin (managed by the bridge; edits are replaced) -->";

pub const RULES_END: &str = "<!-- systemprompt:end -->";

const BUDGET_BYTES: usize = 64 * 1024;

pub(crate) fn select<'m>(
    rules: &'m [RuleEntry],
    host: HostKind,
    warnings: &mut Vec<String>,
) -> Vec<&'m RuleEntry> {
    let mut selected: Vec<&RuleEntry> =
        unique_plain(rules, |rule| rule.id.as_str(), "rule", warnings)
            .into_iter()
            .filter(|rule| rule_targets_host(&rule.hosts, host))
            .collect();
    selected.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
    selected
}

pub(crate) fn block(rules: &[&RuleEntry]) -> Option<String> {
    if rules.is_empty() {
        return None;
    }
    let mut out = String::from(RULES_BEGIN);
    out.push('\n');
    for rule in rules {
        let instructions = rule.instructions.trim();
        out.push_str("\n## ");
        out.push_str(rule.name.as_str());
        out.push_str("\n\n");
        out.push_str(instructions);
        out.push('\n');
    }
    out.push('\n');
    out.push_str(RULES_END);
    out.push('\n');
    Some(out)
}

pub(crate) fn check_budget(block: Option<&str>, warnings: &mut Vec<String>) {
    if let Some(block) = block
        && block.len() > BUDGET_BYTES
    {
        warnings.push(format!(
            "the rules block is {} bytes; the engine reads only the first {} bytes of AGENTS.md",
            block.len(),
            BUDGET_BYTES
        ));
    }
}

pub(crate) fn splice(existing: &str, managed: Option<&str>) -> String {
    let outside = match (existing.find(RULES_BEGIN), existing.find(RULES_END)) {
        (Some(start), Some(stop)) if stop > start => {
            let before = existing.get(..start).unwrap_or_default().trim_end();
            let after = existing
                .get(stop + RULES_END.len()..)
                .unwrap_or_default()
                .trim_start_matches('\n');
            format!("{before}{after}")
        },
        _ => existing.to_owned(),
    };
    let outside = outside.trim_end();
    if outside.is_empty() {
        return managed.unwrap_or_default().to_owned();
    }
    managed.map_or_else(
        || format!("{outside}\n"),
        |block| format!("{outside}\n\n{block}"),
    )
}
