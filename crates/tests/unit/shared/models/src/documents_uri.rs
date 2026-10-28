use systemprompt_identifiers::{DocumentId, DocumentNodeId, DocumentRevisionId};
use systemprompt_models::documents::{DocumentUri, DocumentUriError};

const DOC: &str = "6f1c2c3a-4d55-4f6e-9a01-1b2c3d4e5f60";
const REV: &str = "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";

#[test]
fn display_and_parse_round_trip_all_shapes() {
    let base = DocumentUri::new(DocumentId::new(DOC));
    let shapes = [
        (base.clone(), format!("document://{DOC}")),
        (
            base.clone().at_revision(DocumentRevisionId::new(REV)),
            format!("document://{DOC}@{REV}"),
        ),
        (
            base.clone().at_node(DocumentNodeId::new("p3")),
            format!("document://{DOC}#p3"),
        ),
        (
            base.at_revision(DocumentRevisionId::new(REV))
                .at_node(DocumentNodeId::new("p3")),
            format!("document://{DOC}@{REV}#p3"),
        ),
    ];
    for (uri, text) in shapes {
        assert_eq!(uri.to_string(), text);
        assert_eq!(text.parse::<DocumentUri>().expect("parse"), uri);
        let json = serde_json::to_value(&uri).expect("ser");
        assert_eq!(json, serde_json::json!(text));
    }
}

#[test]
fn parse_rejects_bad_input() {
    assert_eq!(
        "file://x".parse::<DocumentUri>(),
        Err(DocumentUriError::MissingScheme)
    );
    assert!("document://not-a-uuid".parse::<DocumentUri>().is_err());
    assert!(
        format!("document://{DOC}@bad")
            .parse::<DocumentUri>()
            .is_err()
    );
    assert!(format!("document://{DOC}#").parse::<DocumentUri>().is_err());
}
