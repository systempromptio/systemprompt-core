use systemprompt_identifiers::{DocumentNodeId, DocumentTemplateId};
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentCodec, DocumentFormat, DocumentTemplate, DocumentTemplateProvider, EditOp,
    Projection, TemplateParams, TemplateSource, content_hash,
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

#[test]
fn memo_templates_publish_the_memo_template() {
    let templates = MemoTemplates.templates();
    assert_eq!(templates.len(), 1);
    assert_eq!(templates[0].id.as_str(), "memo");
    assert_eq!(templates[0].format, DocumentFormat::Markdown);
}

#[test]
fn default_priority_is_100() {
    assert_eq!(EchoCodec.priority(), 100);
    assert_eq!(MemoTemplates.priority(), 100);
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
