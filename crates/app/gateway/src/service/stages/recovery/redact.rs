//! In-flight redaction of safety findings in `safety.redact_categories`.
//!
//! A finding in a redact category rewrites what it matched in the forwarded
//! request instead of refusing it: each span becomes `[REDACTED:<category>]`,
//! or a scanner-supplied [`PartReplacement`] substitutes the whole part. The
//! body is edited through the same leaf machinery as secret repair, so signed
//! blocks and protocol keys are never touched and the canonical request stays
//! in step. Redaction is all or nothing: a finding that carries no location,
//! names a part outside the forwarded surface, or lands on a leaf that cannot
//! be edited makes the whole request unredactable, and the caller refuses it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::ops::Range;

use systemprompt_wire::canonical::CanonicalRequest;

use super::leaf_edit::{LeafEdit, apply_leaf_edits};
use crate::policies::{Finding, PHASE_RESPONSE, PartReplacement};
use crate::protocol::outbound::PreparedBody;

const SURFACE_ROOT: &str = "$";

#[must_use]
pub fn redaction_marker(category: &str) -> String {
    format!("[REDACTED:{category}]")
}

/// What a successful redaction rewrote: how many body parts, for which
/// categories.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactionReport {
    pub parts: usize,
    pub categories: Vec<String>,
}

/// A finding in a redact category whose content could not be rewritten in the
/// forwarded body. The request must be refused rather than sent unredacted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("content in safety category '{category}' could not be redacted")]
pub struct Unredactable {
    pub category: String,
}

#[derive(Default)]
struct PartEdit {
    replacement: Option<String>,
    spans: Vec<(Range<usize>, String)>,
}

pub fn redact_findings(
    request: &mut CanonicalRequest,
    body: &mut PreparedBody,
    findings: &[Finding],
    redact_categories: &[String],
) -> Result<Option<RedactionReport>, Unredactable> {
    let redacted: Vec<&Finding> = findings
        .iter()
        .filter(|f| {
            f.phase != PHASE_RESPONSE
                && !f.is_scanner_failure()
                && redact_categories.contains(&f.category)
        })
        .collect();
    let Some(first) = redacted.first() else {
        return Ok(None);
    };
    let mut parts: BTreeMap<String, PartEdit> = BTreeMap::new();
    for finding in &redacted {
        collect(finding, &mut parts).ok_or_else(|| unredactable(finding))?;
    }
    let edits: Vec<(String, LeafEdit)> = parts
        .into_iter()
        .map(|(part, edit)| {
            let edit = edit
                .replacement
                .map_or(LeafEdit::Spans(edit.spans), LeafEdit::Replace);
            (part, edit)
        })
        .collect();
    let count = edits.len();
    apply_leaf_edits(request, body, edits).ok_or_else(|| unredactable(first))?;
    let mut categories: Vec<String> = redacted.iter().map(|f| f.category.clone()).collect();
    categories.sort();
    categories.dedup();
    Ok(Some(RedactionReport {
        parts: count,
        categories,
    }))
}

fn collect(finding: &Finding, parts: &mut BTreeMap<String, PartEdit>) -> Option<()> {
    if finding.spans.is_empty() && finding.replacement.is_none() {
        return None;
    }
    if let Some(PartReplacement { part, text }) = &finding.replacement {
        if !on_surface(part) {
            return None;
        }
        parts.entry(part.clone()).or_default().replacement = Some(text.clone());
    }
    let marker = redaction_marker(&finding.category);
    for span in &finding.spans {
        if !on_surface(&span.part) {
            tracing::warn!(
                part = %span.part,
                category = %finding.category,
                "Safety finding located outside the forwarded surface cannot be redacted"
            );
            return None;
        }
        parts
            .entry(span.part.clone())
            .or_default()
            .spans
            .push((span.range.clone(), marker.clone()));
    }
    Some(())
}

fn on_surface(part: &str) -> bool {
    part.starts_with(SURFACE_ROOT)
}

fn unredactable(finding: &Finding) -> Unredactable {
    Unredactable {
        category: finding.category.clone(),
    }
}
