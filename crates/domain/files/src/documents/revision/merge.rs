//! Line-based three-way merge for text formats.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeOutcome {
    Clean(String),
    Conflicted,
}

#[must_use]
pub fn three_way(base: &str, ours: &str, theirs: &str) -> MergeOutcome {
    diffy::merge(base, ours, theirs).map_or(MergeOutcome::Conflicted, MergeOutcome::Clean)
}

#[must_use]
pub fn three_way_bytes(base: &[u8], ours: &[u8], theirs: &[u8]) -> MergeOutcome {
    match (
        std::str::from_utf8(base),
        std::str::from_utf8(ours),
        std::str::from_utf8(theirs),
    ) {
        (Ok(base), Ok(ours), Ok(theirs)) => three_way(base, ours, theirs),
        _ => MergeOutcome::Conflicted,
    }
}
