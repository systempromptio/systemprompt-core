//! The `.odt` codec's `DocumentCodec` implementation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_provider_contracts::documents::{
    CodecError, DocumentCodec, DocumentFormat, EditOp, Projection, TemplateParams,
};

use super::apply::FlowBatch;
use super::{OdtCodec, check_base, finish, instantiate, projection, read, text};

impl DocumentCodec for OdtCodec {
    fn insert_kinds(&self, _format: DocumentFormat) -> &'static [&'static str] {
        &["heading", "paragraph", "list_item", "pending"]
    }

    fn formats(&self) -> &'static [DocumentFormat] {
        &[DocumentFormat::Odt]
    }

    fn project(&self, bytes: &[u8]) -> Result<Projection, CodecError> {
        let (_, content) = read(bytes)?;
        Ok(projection(
            DocumentFormat::Odt,
            bytes,
            text::read(&content)?.nodes(),
        ))
    }

    fn apply(
        &self,
        bytes: &[u8],
        base: &Projection,
        ops: &[EditOp],
    ) -> Result<Vec<u8>, CodecError> {
        check_base(bytes, base)?;
        let (package, content) = read(bytes)?;
        let doc = text::read(&content)?;
        let containers = doc.containers();
        ops.iter().try_for_each(EditOp::check_resolved)?;
        let mut batch = FlowBatch::new(DocumentFormat::Odt, &doc.paras, &containers);
        batch.body_end = content.xml.rfind("</office:text>");
        for (order, op) in ops.iter().enumerate() {
            batch.op(order, op)?;
        }
        finish(package, &content.xml, batch.splices)
    }

    fn preview_html(&self, bytes: &[u8]) -> Result<String, CodecError> {
        Ok(text::read(&read(bytes)?.1)?.preview())
    }

    fn plain_text(&self, bytes: &[u8]) -> Result<String, CodecError> {
        Ok(text::read(&read(bytes)?.1)?.plain_text())
    }

    fn instantiate(&self, template: &[u8], params: &TemplateParams) -> Result<Vec<u8>, CodecError> {
        instantiate::instantiate(template, params)
    }
}
