//! Template instantiation: placeholder substitution in a deck's slide text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use quick_xml::escape::escape;
use systemprompt_provider_contracts::documents::{CodecError, TemplateParams};

use super::projection::Slide;
use super::{paragraph, projection};
use crate::documents::codecs::xml_tree::{Element, parse_xml};
use crate::documents::codecs::zip_package::Package;

fn collect<'e>(element: &'e Element, name: &str, out: &mut Vec<&'e Element>) {
    for child in &element.children {
        if child.name == name {
            out.push(child);
        } else {
            collect(child, name, out);
        }
    }
}

fn substitute(text: &str, params: &TemplateParams, escaped: bool) -> String {
    params
        .values
        .iter()
        .fold(text.to_owned(), |text, (key, value)| {
            let value = if escaped {
                escape(value.as_str()).into_owned()
            } else {
                value.clone()
            };
            text.replace(&format!("{{{{{key}}}}}"), &value)
        })
}

fn instantiate_slide(slide: &Slide, params: &TemplateParams) -> Result<Option<String>, CodecError> {
    let root = parse_xml(&slide.part, &slide.xml)?;
    let mut paragraphs = Vec::new();
    collect(&root, "a:p", &mut paragraphs);
    let mut edits = Vec::new();
    for p in paragraphs {
        let para = projection::paragraph(&slide.part, &slide.xml, p, false)?;
        let mut runs = Vec::new();
        collect(p, "a:t", &mut runs);
        let mut run_texts = Vec::new();
        for run in &runs {
            run_texts.push(run.text(&slide.part, &slide.xml)?);
        }
        let split = params.values.keys().any(|key| {
            let token = format!("{{{{{key}}}}}");
            let in_runs: usize = run_texts
                .iter()
                .map(|text| text.matches(&token).count())
                .sum();
            in_runs != para.text.matches(&token).count()
        });
        if split {
            let text = substitute(&para.text, params, false);
            edits.push((
                p.start,
                p.end,
                paragraph::paragraph_xml(&para, para.ppr.as_deref(), &text),
            ));
            continue;
        }
        for run in runs {
            let raw = &slide.xml[run.inner_start..run.inner_end];
            let replaced = substitute(raw, params, true);
            if replaced != raw {
                edits.push((run.inner_start, run.inner_end, replaced));
            }
        }
    }
    Ok((!edits.is_empty()).then(|| crate::documents::codecs::xml_tree::splice(&slide.xml, edits)))
}

pub(super) fn instantiate(template: &[u8], params: &TemplateParams) -> Result<Vec<u8>, CodecError> {
    let mut package = Package::open(template)?;
    for slide in projection::load(&mut package)? {
        if let Some(xml) = instantiate_slide(&slide, params)? {
            package.write(&slide.part, xml);
        }
    }
    package.finish()
}
