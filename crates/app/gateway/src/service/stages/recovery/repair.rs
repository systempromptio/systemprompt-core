//! Secret repair: the located secret spans in the provider-bound JSON body are
//! replaced by the redaction marker, and the repaired body is re-governed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_security::policy::GovernedInput;
use systemprompt_security::policy::secrets::{REDACTION_MARKER, SecretFinding};
use systemprompt_wire::canonical::CanonicalRequest;
use systemprompt_wire::inspect;

use super::leaf_edit::{LeafEdit, apply_leaf_edits};
use super::{governed_input, inspection_budget};
use crate::protocol::outbound::PreparedBody;

pub fn repair_prompt(
    request: &mut CanonicalRequest,
    body: &mut PreparedBody,
    findings: &[SecretFinding],
) -> Option<GovernedInput> {
    let surface = inspect::string_leaves(&body.bytes, inspection_budget());
    if surface.truncated() || findings.is_empty() {
        return None;
    }
    let mut grouped: HashMap<usize, Vec<_>> = HashMap::new();
    for finding in findings {
        grouped
            .entry(finding.source.part_index)
            .or_default()
            .push((finding.span.clone(), REDACTION_MARKER.to_owned()));
    }
    let mut edits = Vec::with_capacity(grouped.len());
    for (index, spans) in grouped {
        let leaf = surface.leaves().get(index)?;
        edits.push((leaf.path.clone(), LeafEdit::Spans(spans)));
    }
    apply_leaf_edits(request, body, edits)?;
    Some(governed_input(&request.forwarded_surface))
}
