use systemprompt_files::documents::codecs::odf::{OdtCodec, blank_odt};
use systemprompt_provider_contracts::documents::{
    CodecError, ContentAs, DocumentCodec, EditOp, NodeKind, NodeTarget, TemplateParams, hash8,
};

use super::docx_fixture::part;
use super::odf_fixture::{
    apply, assert_mimetype_first_stored, assert_only_content_changed, hash_of, id, package, text_of,
};

const MIME: &str = "application/vnd.oasis.opendocument.text";

const BODY: &str = concat!(
    "<office:text><text:sequence-decls/>",
    r#"<text:h text:outline-level="2" text:style-name="H2">Quarterly <text:span text:style-name="T1">Report</text:span></text:h>"#,
    r#"<text:p text:style-name="P1"><text:span text:style-name="Bold">Revenue</text:span> grew<text:s text:c="2"/>fast<text:tab/>now<text:line-break/>end</text:p>"#,
    r#"<text:p xml:id="para-x">Anchored &amp; named</text:p>"#,
    "<text:list><text:list-item><text:p>First point</text:p></text:list-item>",
    "<text:list-item><text:p>Second point</text:p></text:list-item></text:list>",
    r#"<table:table table:name="T"><table:table-column/><table:table-row>"#,
    "<table:table-cell><text:p>Region</text:p></table:table-cell>",
    "<table:table-cell><text:p>Total</text:p></table:table-cell></table:table-row></table:table>",
    "<text:p>Prepared for {{client}}</text:p></office:text>",
);

fn doc() -> Vec<u8> {
    package(MIME, BODY)
}

#[test]
fn projects_headings_paragraphs_lists_and_tables() {
    let projection = OdtCodec.project(&doc()).expect("project");
    let heading = projection.find(&id("body/p[0]")).expect("heading");
    assert_eq!(heading.kind, NodeKind::Heading { level: 2 });
    assert_eq!(
        heading.hashline(),
        format!("body/p[0]:{}| Quarterly Report", hash8("Quarterly Report"))
    );
    assert_eq!(
        text_of(&projection, "body/p[1]").as_deref(),
        Some("Revenue grew  fast\tnow\nend")
    );
    assert_eq!(
        text_of(&projection, "xid:para-x").as_deref(),
        Some("Anchored & named")
    );
    assert_eq!(
        projection.find(&id("body/p[3]")).map(|n| n.kind),
        Some(NodeKind::ListItem)
    );
    assert_eq!(
        text_of(&projection, "body/table[0]/tr[0]/tc[1]/p[0]").as_deref(),
        Some("Total")
    );
    assert_eq!(
        projection.find(&id("body/table[0]")).map(|n| n.kind),
        Some(NodeKind::Table)
    );
}

#[test]
fn replace_text_changes_only_that_paragraph_in_content_xml() {
    let bytes = doc();
    let base = OdtCodec.project(&bytes).expect("project");
    let op = EditOp::ReplaceText {
        node: NodeTarget::Id(id("body/p[1]")),
        expected_hash: Some(hash_of(&base, "body/p[1]")),
        text: "Rewritten  twice".into(),
        content_as: ContentAs::Text,
    };
    let out = OdtCodec.apply(&bytes, &base, &[op]).expect("apply");
    assert_only_content_changed(&bytes, &out);
    let before = part(&bytes, "content.xml");
    let after = part(&out, "content.xml");
    let start = before
        .find(r#"<text:p text:style-name="P1">"#)
        .expect("start");
    let end = before[start..].find("</text:p>").expect("end") + start + "</text:p>".len();
    let written = r#"<text:p text:style-name="P1"><text:span text:style-name="Bold">Rewritten <text:s/>twice</text:span></text:p>"#;
    assert_eq!(
        after,
        format!("{}{written}{}", &before[..start], &before[end..])
    );
    let next = OdtCodec.project(&out).expect("reproject");
    assert_eq!(
        text_of(&next, "body/p[1]").as_deref(),
        Some("Rewritten  twice")
    );
}

#[test]
fn stale_anchor_names_node_hashes_and_text() {
    let bytes = doc();
    let base = OdtCodec.project(&bytes).expect("project");
    let actual = hash_of(&base, "xid:para-x");
    let op = EditOp::ReplaceText {
        node: NodeTarget::Id(id("xid:para-x")),
        expected_hash: Some("deadbeef".into()),
        text: "x".into(),
        content_as: ContentAs::Text,
    };
    let err = OdtCodec.apply(&bytes, &base, &[op]).expect_err("stale");
    assert!(matches!(err, CodecError::StaleAnchor { .. }));
    let message = err.to_string();
    for piece in [
        "xid:para-x",
        "deadbeef",
        actual.as_str(),
        "Anchored & named",
    ] {
        assert!(message.contains(piece), "{message} lacks {piece}");
    }
}

#[test]
fn insert_and_delete_shift_ids_on_reprojection() {
    let bytes = doc();
    let base = OdtCodec.project(&bytes).expect("project");
    let ops = [
        EditOp::InsertAfter {
            node: NodeTarget::Id(id("body/p[0]")),
            kind: NodeKind::Paragraph,
            text: "Inserted".into(),
            content_as: ContentAs::Text,
        },
        EditOp::Delete {
            node: NodeTarget::Id(id("body/p[3]")),
            expected_hash: Some(hash_of(&base, "body/p[3]")),
        },
        EditOp::InsertAfter {
            node: NodeTarget::Id(id("body/p[4]")),
            kind: NodeKind::ListItem,
            text: "Third point".into(),
            content_as: ContentAs::Text,
        },
    ];
    let out = apply(&OdtCodec, &bytes, &ops);
    assert_only_content_changed(&bytes, &out);
    let next = OdtCodec.project(&out).expect("reproject");
    assert_eq!(text_of(&next, "body/p[1]").as_deref(), Some("Inserted"));
    assert_eq!(text_of(&next, "body/p[4]").as_deref(), Some("Second point"));
    assert_eq!(text_of(&next, "body/p[5]").as_deref(), Some("Third point"));
    assert_eq!(
        next.find(&id("body/p[5]")).map(|n| n.kind),
        Some(NodeKind::ListItem)
    );
    assert!(!part(&out, "content.xml").contains("First point"));
}

#[test]
fn str_replace_reports_ambiguity_and_absence() {
    let bytes = doc();
    let base = OdtCodec.project(&bytes).expect("project");
    let many = OdtCodec.apply(
        &bytes,
        &base,
        &[EditOp::StrReplace {
            old_str: "point".into(),
            new_str: "x".into(),
        }],
    );
    assert!(matches!(
        many,
        Err(CodecError::AmbiguousMatch { matches: 2 })
    ));
    let none = OdtCodec.apply(
        &bytes,
        &base,
        &[EditOp::StrReplace {
            old_str: "zebra".into(),
            new_str: "x".into(),
        }],
    );
    assert!(matches!(none, Err(CodecError::NoMatch)));
    let html = OdtCodec.apply(
        &bytes,
        &base,
        &[EditOp::SetHtml {
            html: "<p/>".into(),
        }],
    );
    assert!(matches!(html, Err(CodecError::UnsupportedOp { .. })));
}

#[test]
fn preview_plain_text_and_instantiate() {
    let bytes = doc();
    let preview = OdtCodec.preview_html(&bytes).expect("preview");
    assert!(preview.contains("<h2>Quarterly Report</h2>"));
    assert!(preview.contains("<ul><li>First point</li><li>Second point</li></ul>"));
    assert!(preview.contains("<td>Region</td>"));
    assert!(
        OdtCodec
            .plain_text(&bytes)
            .expect("text")
            .contains("Region\tTotal")
    );
    let mut params = TemplateParams::default();
    params.values.insert("client".into(), "Acme & Co".into());
    let made = OdtCodec.instantiate(&bytes, &params).expect("instantiate");
    assert_mimetype_first_stored(&made);
    let projection = OdtCodec.project(&made).expect("project");
    assert_eq!(
        text_of(&projection, "body/p[5]").as_deref(),
        Some("Prepared for Acme & Co")
    );
}

#[test]
fn blank_odt_is_a_valid_package() {
    let blank = blank_odt().expect("blank");
    assert_mimetype_first_stored(&blank);
    let projection = OdtCodec.project(&blank).expect("project");
    assert_eq!(text_of(&projection, "body/p[0]").as_deref(), Some(""));
}
