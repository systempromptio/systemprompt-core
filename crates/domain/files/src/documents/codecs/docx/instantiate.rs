//! Template instantiation: `{{key}}` placeholders in `w:t` text.
//!
//! A placeholder inside one `w:t` element is replaced in place, keeping every
//! run. When Word has split a placeholder across runs — it does so whenever
//! spelling, revision or formatting state changes mid-word — the paragraph's
//! runs are merged into one that keeps the first run's formatting.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_provider_contracts::documents::{CodecError, TemplateParams};

use super::markup::{self, Splice};
use super::{parts, scan};

fn fill(text: &str, params: &TemplateParams) -> (String, usize) {
    let mut out = text.to_owned();
    let mut count = 0;
    for (key, value) in &params.values {
        let placeholder = format!("{{{{{key}}}}}");
        let hits = out.matches(placeholder.as_str()).count();
        if hits > 0 {
            count += hits;
            out = out.replace(placeholder.as_str(), value);
        }
    }
    (out, count)
}

pub(super) fn instantiate(template: &[u8], params: &TemplateParams) -> Result<Vec<u8>, CodecError> {
    let xml = parts::read_document(template)?;
    let doc = scan::parse(&xml)?;
    let mut splices = Vec::new();
    for para in doc.paragraphs() {
        let (filled, total) = fill(&para.text(), params);
        if total == 0 {
            continue;
        }
        let segments: Vec<_> = para
            .texts
            .iter()
            .map(|seg| (seg, fill(&seg.text, params)))
            .filter(|(_, (_, count))| *count > 0)
            .collect();
        let in_place: usize = segments.iter().map(|(_, (_, count))| count).sum();
        if in_place == total {
            for (seg, (text, _)) in segments {
                splices.push(Splice {
                    range: seg.range.clone(),
                    text: markup::text_markup(&text),
                    order: splices.len(),
                });
            }
        } else {
            splices.push(Splice {
                range: para.range.clone(),
                text: markup::replace_paragraph(&xml, para, &filled),
                order: splices.len(),
            });
        }
    }
    if splices.is_empty() {
        return Ok(template.to_vec());
    }
    let edited = markup::apply_splices(&xml, splices)?;
    scan::parse(&edited)?;
    parts::replace_part(template, parts::DOCUMENT, edited.as_bytes())
}
