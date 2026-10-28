use std::sync::Arc;

use systemprompt_files::documents::codecs::CodecRegistry;
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentCodec, DocumentFormat, EditOp, Projection, TemplateParams, codec_for,
};
use systemprompt_provider_contracts::register_document_codec;

#[derive(Debug)]
struct FixedCodec(u32);

impl DocumentCodec for FixedCodec {
    fn formats(&self) -> &'static [DocumentFormat] {
        &[DocumentFormat::Odp]
    }

    fn project(&self, _bytes: &[u8]) -> Result<Projection, CodecError> {
        Err(CodecError::Malformed {
            detail: "test codec",
        })
    }

    fn apply(
        &self,
        _bytes: &[u8],
        _base: &Projection,
        _ops: &[EditOp],
    ) -> Result<Vec<u8>, CodecError> {
        Err(CodecError::Malformed {
            detail: "test codec",
        })
    }

    fn preview_html(&self, _bytes: &[u8]) -> Result<String, CodecError> {
        Ok(String::new())
    }

    fn plain_text(&self, _bytes: &[u8]) -> Result<String, CodecError> {
        Ok(String::new())
    }

    fn instantiate(
        &self,
        _template: &[u8],
        _params: &TemplateParams,
    ) -> Result<Vec<u8>, CodecError> {
        Ok(Vec::new())
    }

    fn priority(&self) -> u32 {
        self.0
    }
}

fn low() -> Arc<dyn DocumentCodec> {
    Arc::new(FixedCodec(10_000))
}

fn high() -> Arc<dyn DocumentCodec> {
    Arc::new(FixedCodec(20_000))
}

register_document_codec!(low);
register_document_codec!(high);

#[test]
fn higher_priority_codec_wins() {
    let registry = CodecRegistry::discover();
    let chosen = registry.for_format(DocumentFormat::Odp).expect("odp codec");
    assert_eq!(chosen.priority(), 20_000);
}

#[test]
fn equal_priority_keeps_first_listed() {
    let registry = CodecRegistry::from_codecs([
        low(),
        Arc::new(FixedCodec(10_000)) as Arc<dyn DocumentCodec>,
        high(),
    ]);
    assert_eq!(
        registry
            .for_format(DocumentFormat::Odp)
            .map(|c| c.priority()),
        Some(20_000)
    );
    let first = low();
    let tie = CodecRegistry::from_codecs([Arc::clone(&first), low()]);
    let chosen = tie.for_format(DocumentFormat::Odp).expect("odp");
    assert!(Arc::ptr_eq(&chosen, &first));
}

#[test]
fn core_codecs_cover_text_formats() {
    let registry = CodecRegistry::discover();
    for format in [
        DocumentFormat::Html,
        DocumentFormat::Markdown,
        DocumentFormat::Text,
    ] {
        assert!(registry.formats().contains(&format));
    }
    let html = codec_for(DocumentFormat::Html).expect("html codec");
    assert!(format!("{html:?}").contains("HtmlCodec"));
}
