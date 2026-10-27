use systemprompt_files::documents::codecs::TextCodec;
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentCodec, EditOp, NodeKind, TemplateParams, hash8,
};

use super::{CRLF_TXT, SAMPLE_TXT, apply_str, delete, hash_of, id, insert_after, replace};

#[test]
fn hashline_renders_id_hash_and_text() {
    let projection = TextCodec.project(SAMPLE_TXT).expect("project");
    let line = projection.find(&id("line[3]")).expect("line 3");
    assert_eq!(
        line.hashline(),
        format!("line[3]:{}| charlie", hash8("charlie"))
    );
    assert_eq!(projection.root[0].children.len(), 4);
}

#[test]
fn replace_with_stale_hash_reports_node_hashes_and_text() {
    let base = TextCodec.project(SAMPLE_TXT).expect("project");
    let actual = hash_of(&base, "line[2]");
    let err = TextCodec
        .apply(
            SAMPLE_TXT,
            &base,
            &[replace("line[2]", "deadbeef".to_owned(), "x")],
        )
        .expect_err("stale");
    assert!(matches!(err, CodecError::StaleAnchor { .. }));
    let message = err.to_string();
    for part in ["line[2]", "deadbeef", actual.as_str(), "bravo"] {
        assert!(message.contains(part), "{message} lacks {part}");
    }
}

#[test]
fn replace_changes_only_that_line() {
    let base = TextCodec.project(SAMPLE_TXT).expect("project");
    let out = apply_str(
        &TextCodec,
        SAMPLE_TXT,
        &[replace("line[2]", hash_of(&base, "line[2]"), "BRAVO")],
    );
    assert_eq!(out, "alpha\nBRAVO\ncharlie\ndelta\n");
}

#[test]
fn insert_and_delete_shift_ids_on_reprojection() {
    let base = TextCodec.project(SAMPLE_TXT).expect("project");
    let ops = [
        insert_after("line[1]", NodeKind::Text, "inserted"),
        delete("line[3]", hash_of(&base, "line[3]")),
    ];
    let out = apply_str(&TextCodec, SAMPLE_TXT, &ops);
    assert_eq!(out, "alpha\ninserted\nbravo\ndelta\n");
    let next = TextCodec.project(out.as_bytes()).expect("reproject");
    let text = |node: &str| next.find(&id(node)).and_then(|n| n.text.clone());
    assert_eq!(text("line[2]").as_deref(), Some("inserted"));
    assert_eq!(text("line[3]").as_deref(), Some("bravo"));
    assert_eq!(text("line[4]").as_deref(), Some("delta"));
    assert!(next.find(&id("line[5]")).is_none());
}

#[test]
fn str_replace_reports_no_match_and_ambiguity() {
    let base = TextCodec.project(SAMPLE_TXT).expect("project");
    let none = TextCodec.apply(
        SAMPLE_TXT,
        &base,
        &[EditOp::StrReplace {
            old_str: "zulu".into(),
            new_str: "x".into(),
        }],
    );
    assert!(matches!(none, Err(CodecError::NoMatch)));
    let many = TextCodec.apply(
        SAMPLE_TXT,
        &base,
        &[EditOp::StrReplace {
            old_str: "a\n".into(),
            new_str: "x".into(),
        }],
    );
    assert!(matches!(
        many,
        Err(CodecError::AmbiguousMatch { matches: 2 })
    ));
}

#[test]
fn crlf_and_trailing_newline_are_preserved() {
    let base = TextCodec.project(CRLF_TXT).expect("project");
    let ops = [
        replace("line[2]", hash_of(&base, "line[2]"), "TWO"),
        insert_after("line[3]", NodeKind::Text, "four"),
    ];
    assert_eq!(
        apply_str(&TextCodec, CRLF_TXT, &ops),
        "one\r\nTWO\r\nthree\r\nfour\r\n"
    );
    let set = apply_str(
        &TextCodec,
        CRLF_TXT,
        &[EditOp::SetText {
            text: "a\nb\n".into(),
        }],
    );
    assert_eq!(set, "a\r\nb\r\n");
}

#[test]
fn unsupported_and_unknown_ops_fail() {
    let base = TextCodec.project(SAMPLE_TXT).expect("project");
    let html = TextCodec.apply(SAMPLE_TXT, &base, &[EditOp::SetHtml { html: "<p>".into() }]);
    assert!(matches!(html, Err(CodecError::UnsupportedOp { .. })));
    let unknown = TextCodec.apply(SAMPLE_TXT, &base, &[delete("line[9]", "00000000".into())]);
    assert!(matches!(unknown, Err(CodecError::UnknownNode { .. })));
}

#[test]
fn preview_escapes_and_instantiate_substitutes() {
    let preview = TextCodec.preview_html(b"<b>&").expect("preview");
    assert_eq!(preview, "<pre>&lt;b&gt;&amp;</pre>");
    let mut params = TemplateParams::default();
    params.values.insert("name".into(), "Ada".into());
    let out = TextCodec
        .instantiate(b"Hi {{ name }}, {{missing}}", &params)
        .expect("instantiate");
    assert_eq!(out, b"Hi Ada, {{missing}}");
}
