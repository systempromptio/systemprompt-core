//! `WordprocessingML` fragments the edits write, and the splice that lays them
//! over the original `word/document.xml` text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ops::Range;

use quick_xml::escape::escape;
use systemprompt_provider_contracts::documents::CodecError;

use super::parts::malformed;
use super::scan::Para;

#[derive(Debug)]
pub(super) struct Splice {
    pub(super) range: Range<usize>,
    pub(super) text: String,
    pub(super) order: usize,
}

pub(super) fn apply_splices(src: &str, mut splices: Vec<Splice>) -> Result<String, CodecError> {
    splices.sort_by(|a, b| {
        b.range
            .start
            .cmp(&a.range.start)
            .then(b.order.cmp(&a.order))
    });
    let mut out = src.to_owned();
    let mut limit = src.len();
    for splice in splices {
        if splice.range.end > limit {
            return Err(malformed("two operations in one batch edit the same node"));
        }
        out.replace_range(splice.range.clone(), &splice.text);
        limit = splice.range.start;
    }
    Ok(out)
}

pub(super) fn text_markup(text: &str) -> String {
    let mut out = String::new();
    let mut pending = String::new();
    for ch in text.chars() {
        match ch {
            '\t' | '\n' => {
                flush_text(&mut out, &mut pending);
                out.push_str(if ch == '\t' { "<w:tab/>" } else { "<w:br/>" });
            },
            _ => pending.push(ch),
        }
    }
    flush_text(&mut out, &mut pending);
    out
}

fn flush_text(out: &mut String, pending: &mut String) {
    if pending.is_empty() {
        return;
    }
    let preserve =
        pending.starts_with(char::is_whitespace) || pending.ends_with(char::is_whitespace);
    out.push_str(if preserve {
        "<w:t xml:space=\"preserve\">"
    } else {
        "<w:t>"
    });
    out.push_str(&escape(pending.as_str()));
    out.push_str("</w:t>");
    pending.clear();
}

pub(super) fn run(rpr: Option<&str>, text: &str) -> String {
    format!(
        "<w:r>{}{}</w:r>",
        rpr.unwrap_or_default(),
        text_markup(text)
    )
}

pub(super) fn replace_paragraph(src: &str, para: &Para, text: &str) -> String {
    let rpr = para.first_rpr.clone().map(|range| &src[range]);
    let mut out = para.open_tag.clone();
    if let Some(props) = &para.ppr {
        out.push_str(&src[props.range.clone()]);
    }
    let first_removable = para.children.iter().position(|child| child.removable);
    for (index, child) in para.children.iter().enumerate() {
        if Some(index) == first_removable {
            out.push_str(&run(rpr, text));
        }
        if !child.removable {
            out.push_str(&src[child.range.clone()]);
        }
    }
    if first_removable.is_none() {
        out.push_str(&run(rpr, text));
    }
    out.push_str("</w:p>");
    out
}

pub(super) fn empty_paragraph(src: &str, para: &Para) -> String {
    let props = para
        .ppr
        .as_ref()
        .map_or("", |props| &src[props.range.clone()]);
    format!("{}{props}</w:p>", para.open_tag)
}

pub(super) fn ppr_without(src: &str, range: &Range<usize>, cut: &[Range<usize>]) -> String {
    let mut cut: Vec<&Range<usize>> = cut.iter().collect();
    cut.sort_by_key(|r| r.start);
    let mut out = String::new();
    let mut at = range.start;
    for r in cut {
        out.push_str(&src[at..r.start]);
        at = r.end;
    }
    out.push_str(&src[at..range.end]);
    out
}
