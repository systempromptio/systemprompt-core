use systemprompt_files::documents::codecs::odf::{OdpCodec, blank_odp};
use systemprompt_provider_contracts::documents::{
    CodecError, ContentAs, DocumentCodec, EditOp, NodeKind, NodeTarget,
};

use super::docx_fixture::part;
use super::odf_fixture::{
    apply, assert_mimetype_first_stored, assert_only_content_changed, hash_of, id, package, text_of,
};

const MIME: &str = "application/vnd.oasis.opendocument.presentation";

const BODY: &str = concat!(
    "<office:presentation>",
    r#"<draw:page draw:name="intro" draw:master-page-name="Default"><draw:frame presentation:class="title" svg:x="3cm" draw:id="f1"><draw:text-box><text:p>Welcome</text:p></draw:text-box></draw:frame>"#,
    r#"<draw:frame presentation:class="outline"><draw:text-box><text:list><text:list-item><text:p>Point A</text:p></text:list-item></text:list><text:p>Plain B</text:p></draw:text-box></draw:frame></draw:page>"#,
    r#"<draw:page draw:name="second" draw:master-page-name="Default"><draw:frame presentation:class="title"><draw:text-box><text:p>Second</text:p></draw:text-box></draw:frame>"#,
    r#"<draw:frame><draw:image xlink:href="Pictures/x.png"/></draw:frame></draw:page>"#,
    "</office:presentation>",
);

fn deck() -> Vec<u8> {
    package(MIME, BODY)
}

fn slide_names(bytes: &[u8]) -> Vec<String> {
    OdpCodec
        .project(bytes)
        .expect("project")
        .root
        .iter()
        .map(|slide| slide.text.clone().unwrap_or_default())
        .collect()
}

#[test]
fn projects_slides_shapes_and_paragraphs_in_order() {
    let projection = OdpCodec.project(&deck()).expect("project");
    assert_eq!(slide_names(&deck()), vec!["intro", "second"]);
    let title = projection
        .find(&id("slide[1]/shape[0]"))
        .expect("title shape");
    assert_eq!(
        title.attrs.get("placeholder").map(String::as_str),
        Some("title")
    );
    assert_eq!(
        text_of(&projection, "slide[1]/shape[0]/p[0]").as_deref(),
        Some("Welcome")
    );
    let point = projection
        .find(&id("slide[1]/shape[1]/p[0]"))
        .expect("point");
    assert_eq!(point.kind, NodeKind::ListItem);
    assert_eq!(
        text_of(&projection, "slide[1]/shape[1]/p[1]").as_deref(),
        Some("Plain B")
    );
    let image = projection.find(&id("slide[2]/shape[1]")).expect("image");
    assert_eq!(image.kind, NodeKind::Image);
    assert_eq!(
        image.attrs.get("href").map(String::as_str),
        Some("Pictures/x.png")
    );
}

#[test]
fn replace_text_touches_only_content_xml() {
    let bytes = deck();
    let base = OdpCodec.project(&bytes).expect("project");
    let op = EditOp::ReplaceText {
        node: NodeTarget::Id(id("slide[1]/shape[0]/p[0]")),
        expected_hash: Some(hash_of(&base, "slide[1]/shape[0]/p[0]")),
        text: "Hello".into(),
        content_as: ContentAs::Text,
    };
    let out = OdpCodec.apply(&bytes, &base, &[op]).expect("apply");
    assert_only_content_changed(&bytes, &out);
    let next = OdpCodec.project(&out).expect("reproject");
    assert_eq!(
        text_of(&next, "slide[1]/shape[0]/p[0]").as_deref(),
        Some("Hello")
    );
    let stale = OdpCodec.apply(
        &bytes,
        &base,
        &[EditOp::Delete {
            node: NodeTarget::Id(id("slide[2]")),
            expected_hash: Some("00000000".into()),
        }],
    );
    assert!(matches!(stale, Err(CodecError::StaleAnchor { .. })));
}

#[test]
fn insert_slide_keeps_order_and_copies_the_reference_frames() {
    let bytes = deck();
    let op = EditOp::InsertSlide {
        after: Some(id("slide[1]")),
        layout: None,
        title: Some("Middle".into()),
        body: vec!["One".into(), "Two".into()],
    };
    let out = apply(&OdpCodec, &bytes, &[op]);
    assert_only_content_changed(&bytes, &out);
    assert_eq!(slide_names(&out), vec!["intro", "page3", "second"]);
    let next = OdpCodec.project(&out).expect("reproject");
    assert_eq!(
        text_of(&next, "slide[2]/shape[0]/p[0]").as_deref(),
        Some("Middle")
    );
    assert_eq!(
        text_of(&next, "slide[2]/shape[1]/p[1]").as_deref(),
        Some("Two")
    );
    assert_eq!(
        next.find(&id("slide[2]"))
            .and_then(|s| s.attrs.get("master").cloned())
            .as_deref(),
        Some("Default")
    );
    let content = part(&out, "content.xml");
    assert_eq!(content.matches("draw:id=\"f1\"").count(), 1);
    assert_eq!(content.matches("svg:x=\"3cm\"").count(), 2);
}

#[test]
fn insert_slide_rejects_an_unknown_master_and_delete_removes_a_slide() {
    let bytes = deck();
    let base = OdpCodec.project(&bytes).expect("project");
    let bad = OdpCodec.apply(
        &bytes,
        &base,
        &[EditOp::InsertSlide {
            after: None,
            layout: Some("Missing".into()),
            title: None,
            body: Vec::new(),
        }],
    );
    assert!(matches!(bad, Err(CodecError::Malformed { .. })));
    let out = apply(
        &OdpCodec,
        &bytes,
        &[EditOp::Delete {
            node: NodeTarget::Id(id("slide[1]")),
            expected_hash: Some(hash_of(&base, "slide[1]")),
        }],
    );
    assert_eq!(slide_names(&out), vec!["second"]);
}

#[test]
fn preview_plain_text_and_blank() {
    let preview = OdpCodec.preview_html(&deck()).expect("preview");
    assert!(preview.contains("<section data-slide=\"1\"><h2>Welcome</h2>"));
    assert!(preview.contains("<li>Point A</li>"));
    assert!(
        OdpCodec
            .plain_text(&deck())
            .expect("text")
            .starts_with("# Slide 1\nWelcome\n")
    );
    let blank = blank_odp().expect("blank");
    assert_mimetype_first_stored(&blank);
    let projection = OdpCodec.project(&blank).expect("project");
    assert_eq!(projection.root.len(), 1);
    assert!(projection.find(&id("slide[1]/shape[0]/p[0]")).is_some());
}
