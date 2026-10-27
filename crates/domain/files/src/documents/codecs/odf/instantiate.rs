//! `{{key}}` placeholder substitution in the text of every paragraph of
//! `content.xml`, shared by the three OpenDocument codecs.
//!
//! A placeholder that sits inside one text node is replaced in place, so the
//! paragraph keeps its spans; one split across spans rewrites the paragraph
//! with its first span's style.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use quick_xml::escape::escape;
use systemprompt_provider_contracts::documents::{CodecError, TemplateParams};

use super::package::{CONTENT, Content, Splice, apply_splices, substitute};
use super::projection::{Flow, is_para, replaced};
use crate::documents::codecs::xml_tree::{Element, parse_xml};
use crate::documents::codecs::zip_package::Package;

fn outer_paragraphs<'e>(element: &'e Element, out: &mut Vec<&'e Element>) {
    for child in &element.children {
        if is_para(child) {
            out.push(child);
        } else {
            outer_paragraphs(child, out);
        }
    }
}

pub(super) fn instantiate(template: &[u8], params: &TemplateParams) -> Result<Vec<u8>, CodecError> {
    let mut package = Package::open(template)?;
    let content = Content::read(&mut package)?;
    let Some(body) = content.root.child("office:body") else {
        return package.finish();
    };
    let mut paragraphs = Vec::new();
    outer_paragraphs(body, &mut paragraphs);
    let mut splices = Vec::new();
    let mut flow = Flow::new(&content.xml, "instantiate");
    for element in paragraphs {
        flow.para(element, None)?;
    }
    for para in &flow.paras {
        let filled = substitute(&para.text, params);
        if filled == para.text {
            continue;
        }
        let raw = &content.xml[para.range.clone()];
        let in_place = params.values.keys().all(|key| {
            let token = format!("{{{{{key}}}}}");
            raw.matches(&token).count() == para.text.matches(&token).count()
        });
        let text = if in_place {
            params
                .values
                .iter()
                .fold(raw.to_owned(), |raw, (key, value)| {
                    raw.replace(&format!("{{{{{key}}}}}"), &escape(value.as_str()))
                })
        } else {
            replaced(para, &filled)
        };
        splices.push(Splice {
            range: para.range.clone(),
            text,
            order: 0,
        });
    }
    if splices.is_empty() {
        return package.finish();
    }
    let edited = apply_splices(&content.xml, splices)?;
    parse_xml(CONTENT, &edited)?;
    package.write(CONTENT, edited);
    package.finish()
}
