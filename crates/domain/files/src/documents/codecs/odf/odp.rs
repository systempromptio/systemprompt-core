//! The `.odp` codec's `DocumentCodec` implementation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentCodec, DocumentFormat, EditOp, Projection, TemplateParams,
};

use crate::documents::codecs::fragments::leaf_hashes;

use super::apply::FlowBatch;
use super::package::malformed;
use super::{OdpCodec, check_base, deck, finish, fragment, instantiate, projection, read};
use crate::documents::codecs::xml_tree::parse_xml;
use crate::documents::codecs::zip_package::Package;

fn master_pages(package: &mut Package<'_>) -> Result<Vec<String>, CodecError> {
    let Some(xml) = package.read("styles.xml")? else {
        return Ok(Vec::new());
    };
    let root = parse_xml("styles.xml", &xml)?;
    Ok(root
        .child("office:master-styles")
        .map(|styles| {
            styles
                .children_named("style:master-page")
                .filter_map(|page| page.attr("style:name").map(str::to_owned))
                .collect()
        })
        .unwrap_or_default())
}

impl DocumentCodec for OdpCodec {
    fn insert_kinds(&self, _format: DocumentFormat) -> &'static [&'static str] {
        &["heading", "paragraph", "list_item", "pending"]
    }

    fn formats(&self) -> &'static [DocumentFormat] {
        &[DocumentFormat::Odp]
    }

    fn project(&self, bytes: &[u8]) -> Result<Projection, CodecError> {
        let (_, content) = read(bytes)?;
        Ok(projection(
            DocumentFormat::Odp,
            bytes,
            deck::read(&content)?.nodes(),
        ))
    }

    fn apply(
        &self,
        bytes: &[u8],
        base: &Projection,
        ops: &[EditOp],
    ) -> Result<Vec<u8>, CodecError> {
        check_base(bytes, base)?;
        let (mut package, content) = read(bytes)?;
        let deck = deck::read(&content)?;
        let containers = deck.containers();
        ops.iter().try_for_each(EditOp::check_resolved)?;
        let mut batch = FlowBatch::new(DocumentFormat::Odp, &deck.paras, &containers);
        for (order, op) in ops.iter().enumerate() {
            if let EditOp::InsertSlide {
                after,
                layout,
                title,
                body,
            } = op
            {
                if let Some(layout) = layout
                    && !master_pages(&mut package)?.contains(layout)
                {
                    return Err(malformed(
                        "insert_slide names a master page styles.xml does not define",
                    ));
                }
                let (at, page) = deck.new_page(
                    after.as_ref().map(DocumentNodeId::as_str),
                    layout.as_deref(),
                    title.as_deref(),
                    body,
                )?;
                batch.push(order, at..at, page);
            } else {
                batch.op(order, op)?;
            }
        }
        finish(package, &content.xml, batch.splices)
    }

    fn preview_html(&self, bytes: &[u8]) -> Result<String, CodecError> {
        Ok(deck::read(&read(bytes)?.1)?.preview())
    }

    fn plain_text(&self, bytes: &[u8]) -> Result<String, CodecError> {
        Ok(deck::read(&read(bytes)?.1)?.plain_text())
    }

    fn instantiate(&self, template: &[u8], params: &TemplateParams) -> Result<Vec<u8>, CodecError> {
        instantiate::instantiate(template, params)
    }

    fn style_hashes(&self, bytes: &[u8]) -> Result<Vec<(DocumentNodeId, String)>, CodecError> {
        let (_, content) = read(bytes)?;
        let paras = deck::read(&content)?.paras;
        Ok(leaf_hashes(
            &self.project(bytes)?,
            &fragment::styled(&content.xml, &paras)?,
        ))
    }
}
