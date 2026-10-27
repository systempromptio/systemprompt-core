//! The bridge-owned block of `OpenCode`'s `AGENTS.md`.
//!
//! The block holds the rules whose `hosts` name an `OpenCode` packaging,
//! ordered by id. The bridge writes it and `services validate` measures it,
//! so both see the same text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::bridge::host::HostProduct;
use crate::bridge::manifest::RuleEntry;

pub const BLOCK_BEGIN: &str =
    "<!-- systemprompt:begin (managed by the bridge; edits are replaced) -->";
pub const BLOCK_END: &str = "<!-- systemprompt:end -->";

// Why: OpenCode stops reading AGENTS.md instructions past this many
// characters, so rules beyond it never reach the model.
pub const AGENTS_MD_BUDGET_CHARS: usize = 24_000;

#[must_use]
pub fn rule_targets_opencode(rule: &RuleEntry) -> bool {
    HostProduct::OpenCode.targeted_by(&rule.hosts)
}

#[must_use]
pub fn agents_block(rules: &[RuleEntry]) -> Option<String> {
    let mut selected: Vec<&RuleEntry> = rules.iter().filter(|r| rule_targets_opencode(r)).collect();
    if selected.is_empty() {
        return None;
    }
    selected.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
    let mut block = format!("{BLOCK_BEGIN}\n");
    for rule in selected {
        block.push_str(&format!(
            "\n## {}\n\n{}\n",
            rule.name.as_str(),
            rule.instructions.trim()
        ));
    }
    block.push_str(&format!("\n{BLOCK_END}\n"));
    Some(block)
}

/// The block's length in characters when it exceeds
/// [`AGENTS_MD_BUDGET_CHARS`].
#[must_use]
pub fn over_budget(rules: &[RuleEntry]) -> Option<usize> {
    agents_block(rules)
        .map(|block| block.chars().count())
        .filter(|chars| *chars > AGENTS_MD_BUDGET_CHARS)
}

#[must_use]
pub fn splice_block(existing: &str, block: Option<&str>) -> String {
    let outside = match (existing.find(BLOCK_BEGIN), existing.find(BLOCK_END)) {
        (Some(start), Some(end)) if end > start => {
            let tail = &existing[end + BLOCK_END.len()..];
            format!(
                "{}{}",
                existing[..start].trim_end(),
                tail.trim_start_matches('\n')
            )
        },
        _ => existing.to_owned(),
    };
    let outside = outside.trim_end();
    match (outside.is_empty(), block) {
        (true, None) => String::new(),
        (true, Some(block)) => block.to_owned(),
        (false, None) => format!("{outside}\n"),
        (false, Some(block)) => format!("{outside}\n\n{block}"),
    }
}
