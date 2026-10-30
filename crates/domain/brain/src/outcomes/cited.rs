//! Citation scan: which injected fact ids an assistant response names as
//! whole tokens.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::FactRef;

const fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

#[must_use]
pub fn mentions(text: &str, id: &str) -> bool {
    if id.is_empty() {
        return false;
    }
    text.match_indices(id).any(|(start, _)| {
        let before = text[..start].chars().next_back();
        let after = text[start + id.len()..].chars().next();
        !before.is_some_and(is_token_char) && !after.is_some_and(is_token_char)
    })
}

#[must_use]
pub fn cited_facts(text: &str, injected: &[FactRef]) -> Vec<BrainFactId> {
    let mut cited: Vec<BrainFactId> = Vec::new();
    for fact in injected {
        if !cited.contains(&fact.id) && mentions(text, fact.id.as_str()) {
            cited.push(fact.id.clone());
        }
    }
    cited
}
