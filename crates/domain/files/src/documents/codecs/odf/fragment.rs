//! Formatting-sensitive leaf hashes for the text and presentation codecs:
//! a paragraph's style hash covers the `text:style-name` of the paragraph
//! and of every nested span, and its text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_provider_contracts::documents::CodecError;

use super::package::CONTENT;
use super::projection::Para;
use crate::documents::codecs::xml_tree::{Element, parse_xml};

fn style_names(element: &Element, out: &mut String) {
    if let Some(style) = element.attr("text:style-name") {
        out.push_str(&format!("{}={style};", element.name));
    }
    for child in &element.children {
        style_names(child, out);
    }
}

pub(super) fn styled(xml: &str, paras: &[Para]) -> Result<HashMap<String, String>, CodecError> {
    paras
        .iter()
        .map(|para| {
            let mut out = String::new();
            style_names(&parse_xml(CONTENT, &xml[para.range.clone()])?, &mut out);
            out.push('\u{1}');
            out.push_str(&para.text);
            Ok((para.id.clone(), out))
        })
        .collect()
}
