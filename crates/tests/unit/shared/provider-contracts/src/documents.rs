use std::sync::Arc;

use systemprompt_identifiers::{DocumentNodeId, DocumentTemplateId};
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentCodec, DocumentFormat, DocumentTemplate, DocumentTemplateProvider, EditOp,
    Projection, TemplateParams, TemplateSource, codec_for, content_hash, registered_codecs,
    registered_template_providers,
};
use systemprompt_provider_contracts::{
    register_document_codec, register_document_template_provider,
};

#[derive(Debug)]
struct EchoCodec;

impl DocumentCodec for EchoCodec {
    fn formats(&self) -> &'static [DocumentFormat] {
        &[DocumentFormat::Text]
    }

    fn project(&self, bytes: &[u8]) -> Result<Projection, CodecError> {
        Ok(Projection {
            format: DocumentFormat::Text,
            root: Vec::new(),
            content_sha256: content_hash(bytes),
        })
    }

    fn apply(&self, bytes: &[u8], _: &Projection, _: &[EditOp]) -> Result<Vec<u8>, CodecError> {
        Ok(bytes.to_vec())
    }

    fn preview_html(&self, _: &[u8]) -> Result<String, CodecError> {
        Ok(String::new())
    }

    fn plain_text(&self, bytes: &[u8]) -> Result<String, CodecError> {
        Ok(String::from_utf8_lossy(bytes).into_owned())
    }

    fn instantiate(&self, template: &[u8], _: &TemplateParams) -> Result<Vec<u8>, CodecError> {
        Ok(template.to_vec())
    }
}

#[derive(Debug)]
struct PreferredCodec;

impl DocumentCodec for PreferredCodec {
    fn formats(&self) -> &'static [DocumentFormat] {
        &[DocumentFormat::Text]
    }

    fn project(&self, bytes: &[u8]) -> Result<Projection, CodecError> {
        EchoCodec.project(bytes)
    }

    fn apply(&self, bytes: &[u8], b: &Projection, o: &[EditOp]) -> Result<Vec<u8>, CodecError> {
        EchoCodec.apply(bytes, b, o)
    }

    fn preview_html(&self, bytes: &[u8]) -> Result<String, CodecError> {
        EchoCodec.preview_html(bytes)
    }

    fn plain_text(&self, _: &[u8]) -> Result<String, CodecError> {
        Ok("preferred".into())
    }

    fn instantiate(&self, t: &[u8], p: &TemplateParams) -> Result<Vec<u8>, CodecError> {
        EchoCodec.instantiate(t, p)
    }

    fn priority(&self) -> u32 {
        200
    }
}

#[derive(Debug)]
struct MemoTemplates;

impl DocumentTemplateProvider for MemoTemplates {
    fn provider_id(&self) -> &'static str {
        "test-memo"
    }

    fn templates(&self) -> Vec<DocumentTemplate> {
        vec![DocumentTemplate {
            id: DocumentTemplateId::new("memo"),
            format: DocumentFormat::Markdown,
            name: "Memo".into(),
            description: "A memo".into(),
            source: TemplateSource::Embedded(b"# {{title}}"),
            placeholders: Vec::new(),
        }]
    }
}

fn echo() -> Arc<dyn DocumentCodec> {
    Arc::new(EchoCodec)
}

fn preferred() -> Arc<dyn DocumentCodec> {
    Arc::new(PreferredCodec)
}

fn memo() -> Arc<dyn DocumentTemplateProvider> {
    Arc::new(MemoTemplates)
}

register_document_codec!(echo);
register_document_codec!(preferred);
register_document_template_provider!(memo);

#[test]
fn registered_codecs_are_discoverable_highest_priority_first() {
    let priorities: Vec<u32> = registered_codecs().map(|c| c.priority()).collect();
    assert!(priorities.contains(&100));
    assert!(priorities.contains(&200));
    assert!(priorities.windows(2).all(|w| w[0] >= w[1]));
}

#[test]
fn codec_for_picks_highest_priority() {
    let codec = codec_for(DocumentFormat::Text).expect("text codec");
    assert_eq!(codec.plain_text(b"x").expect("text"), "preferred");
    assert!(
        registered_codecs().any(|c| c.priority() == 100),
        "the lower-priority codec stays registered behind the preferred one"
    );
}

#[test]
fn default_priority_is_100() {
    assert_eq!(EchoCodec.priority(), 100);
    assert_eq!(MemoTemplates.priority(), 100);
}

#[test]
fn registered_template_providers_are_discoverable() {
    let provider = registered_template_providers()
        .find(|p| p.provider_id() == "test-memo")
        .expect("registered");
    assert_eq!(provider.templates()[0].id.as_str(), "memo");
}

#[test]
fn stale_anchor_message_names_node_hashes_and_text() {
    let err = CodecError::StaleAnchor {
        node: DocumentNodeId::new("p4"),
        expected: "aaaa1111".into(),
        actual: "bbbb2222".into(),
        current_text: "Revenue grew".into(),
    };
    assert_eq!(
        err.to_string(),
        "node p4 has changed since it was read (expected aaaa1111, now bbbb2222); current text: \
         \"Revenue grew\""
    );
}

#[test]
fn codec_error_messages_name_their_subject() {
    let unknown = CodecError::UnknownNode {
        node: DocumentNodeId::new("p9"),
    };
    assert!(unknown.to_string().contains("p9"));
    let unsupported = CodecError::UnsupportedOp {
        op: "set_cells",
        format: DocumentFormat::Docx,
    };
    assert!(unsupported.to_string().contains("set_cells"));
    assert!(unsupported.to_string().contains("docx"));
    assert!(
        CodecError::AmbiguousMatch { matches: 3 }
            .to_string()
            .contains('3')
    );
}
