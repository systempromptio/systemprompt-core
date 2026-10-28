//! The page `insert_slide` writes into an `.odp` deck.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::CodecError;

use super::deck::{Deck, Slide};
use super::package::{attr_escaped, text_markup};

impl Deck {
    fn reference(&self, after: Option<&str>) -> Result<(Option<&Slide>, usize), CodecError> {
        after.map_or_else(
            || Ok((self.slides.last(), self.insert_at)),
            |id| {
                self.slides
                    .iter()
                    .find(|slide| slide.id == id)
                    .map(|slide| (Some(slide), slide.range.end))
                    .ok_or_else(|| CodecError::UnknownNode {
                        node: DocumentNodeId::new(id),
                    })
            },
        )
    }

    fn fresh_name(&self) -> String {
        let mut number = self.slides.len() + 1;
        loop {
            let candidate = format!("page{number}");
            if !self
                .slides
                .iter()
                .any(|s| s.name.as_deref() == Some(candidate.as_str()))
            {
                return candidate;
            }
            number += 1;
        }
    }

    pub(super) fn new_page(
        &self,
        after: Option<&str>,
        master: Option<&str>,
        title: Option<&str>,
        body: &[String],
    ) -> Result<(usize, String), CodecError> {
        let (reference, at) = self.reference(after)?;
        let mut page = format!("<draw:page draw:name=\"{}\"", self.fresh_name());
        let master = master
            .map(str::to_owned)
            .or_else(|| reference.and_then(|s| s.master.clone()));
        if let Some(master) = master {
            page.push_str(&format!(
                " draw:master-page-name=\"{}\"",
                attr_escaped(&master)
            ));
        }
        if let Some(layout) = reference.and_then(|s| s.layout.as_deref()) {
            page.push_str(&format!(
                " presentation:presentation-page-layout-name=\"{}\"",
                attr_escaped(layout)
            ));
        }
        page.push('>');
        if let Some(title) = title {
            page.push_str(&frame(reference, "title", &[title.to_owned()]));
        }
        if !body.is_empty() {
            page.push_str(&frame(reference, "outline", body));
        }
        page.push_str("</draw:page>");
        Ok((at, page))
    }
}

fn frame(reference: Option<&Slide>, class: &str, paras: &[String]) -> String {
    let open = reference
        .and_then(|slide| {
            slide
                .shapes
                .iter()
                .find(|s| s.placeholder.as_deref() == Some(class))
        })
        .map_or_else(
            || {
                let (y, height) = if class == "title" {
                    ("1cm", "3cm")
                } else {
                    ("5cm", "12cm")
                };
                format!(
                    "<draw:frame presentation:class=\"{class}\" svg:x=\"2cm\" svg:y=\"{y}\" \
                     svg:width=\"24cm\" svg:height=\"{height}\">"
                )
            },
            |shape| strip_ids(&shape.open),
        );
    let mut text = String::new();
    for para in paras {
        text.push_str("<text:p>");
        text.push_str(&text_markup(para));
        text.push_str("</text:p>");
    }
    format!("{open}<draw:text-box>{text}</draw:text-box></draw:frame>")
}

fn strip_ids(tag: &str) -> String {
    let mut out = tag.to_owned();
    for name in ["xml:id=", "draw:id=", "draw:name="] {
        while let Some(at) = out.find(&format!(" {name}")) {
            let value_start = at + name.len() + 1;
            let quote = out[value_start..].chars().next().unwrap_or('"');
            let close = out[value_start + 1..]
                .find(quote)
                .map_or(out.len(), |i| value_start + 2 + i);
            out.replace_range(at..close, "");
        }
    }
    out
}
