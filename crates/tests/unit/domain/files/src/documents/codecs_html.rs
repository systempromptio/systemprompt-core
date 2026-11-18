use systemprompt_files::documents::codecs::HtmlCodec;
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentCodec, EditOp, NodeKind, TemplateParams,
};

use super::{SAMPLE_HTML, apply_str, delete, hash_of, id, insert_after, replace};

#[test]
fn projection_ids_follow_dom_order() {
    let projection = HtmlCodec.project(SAMPLE_HTML).expect("project");
    let text = |node: &str| projection.find(&id(node)).and_then(|n| n.text.clone());
    assert_eq!(text("body/h1[1]").as_deref(), Some("Heading"));
    assert_eq!(text("body/p[1]").as_deref(), Some("First bold para"));
    assert_eq!(text("body/p[2]").as_deref(), Some("Second para"));
    assert_eq!(text("body/li[2]").as_deref(), Some("two"));
    assert_eq!(text("body/td[2]").as_deref(), Some("b"));
    assert_eq!(text("body/img[1]").as_deref(), Some("Logo"));
    let table = projection.find(&id("body/table[1]")).expect("table");
    assert_eq!(table.kind, NodeKind::Table);
    assert_eq!(table.children[0].children.len(), 2);
}

#[test]
fn replace_text_on_second_paragraph_keeps_head_and_attributes() {
    let base = HtmlCodec.project(SAMPLE_HTML).expect("project");
    let out = apply_str(
        &HtmlCodec,
        SAMPLE_HTML,
        &[replace("body/p[2]", hash_of(&base, "body/p[2]"), "Changed")],
    );
    assert!(out.contains("<p>Changed</p>"));
    assert!(!out.contains("Second para"));
    assert!(out.contains("<title>Keep Me</title><style>p{color:red}</style>"));
    assert!(out.contains(r#"<body class="page" data-x="1">"#));
    assert!(out.contains(r#"<p class="lead">First <strong>bold</strong> para</p>"#));
    assert!(out.starts_with("<!DOCTYPE html>"));
}

#[test]
fn insert_and_delete_reproject() {
    let base = HtmlCodec.project(SAMPLE_HTML).expect("project");
    let ops = [
        insert_after("body/h1[1]", NodeKind::Paragraph, "Intro <x>"),
        delete("body/li[1]", hash_of(&base, "body/li[1]")),
    ];
    let out = apply_str(&HtmlCodec, SAMPLE_HTML, &ops);
    assert!(out.contains("</h1><p>Intro &lt;x&gt;</p>"));
    let next = HtmlCodec.project(out.as_bytes()).expect("reproject");
    let text = |node: &str| next.find(&id(node)).and_then(|n| n.text.clone());
    assert_eq!(text("body/p[1]").as_deref(), Some("Intro <x>"));
    assert_eq!(text("body/p[3]").as_deref(), Some("Second para"));
    assert_eq!(text("body/li[1]").as_deref(), Some("two"));
}

const PLAIN: &[u8] = b"<!DOCTYPE html><html><head><title>Keep Me</title>\
<script>boot()</script></head><body><h1>Heading</h1><p>First <a href=\"https://example.com/\">link</a> \
para</p><p>Second para</p></body></html>";

const SCRIPTED: &[u8] = b"<!DOCTYPE html><html><head><title>App</title></head><body>\
<h1>Heading</h1><p>Second para</p><script>window.ready = 1</script></body></html>";

fn applied(bytes: &[u8], ops: &[EditOp]) -> Result<String, CodecError> {
    let base = HtmlCodec.project(bytes).expect("project");
    HtmlCodec
        .apply(bytes, &base, ops)
        .map(|out| String::from_utf8(out).expect("utf8"))
}

fn str_replace(old: &str, new: &str) -> EditOp {
    EditOp::StrReplace {
        old_str: old.to_owned(),
        new_str: new.to_owned(),
    }
}

fn stripped(result: Result<String, CodecError>) {
    match result {
        Err(CodecError::Refused { detail }) => {
            assert!(detail.contains("content_blob_ref"), "{detail}");
        },
        other => panic!("expected a refusal naming content_blob_ref, got {other:?}"),
    }
}

#[test]
fn set_html_with_markup_the_sanitiser_drops_is_refused() {
    stripped(applied(
        SAMPLE_HTML,
        &[EditOp::SetHtml {
            html: r#"<p onclick="x()">Safe</p><script>alert(1)</script>"#.into(),
        }],
    ));
}

#[test]
fn set_html_of_clean_markup_lands_and_keeps_the_head() {
    let out = applied(
        SAMPLE_HTML,
        &[EditOp::SetHtml {
            html: r#"<p>Safe <a href="https://example.com/">link</a></p>"#.into(),
        }],
    )
    .expect("clean markup");
    assert!(
        out.contains(r#"<p>Safe <a href="https://example.com/">link</a></p>"#),
        "{out}"
    );
    assert!(!out.contains("Second para"));
    assert!(out.contains("<title>Keep Me</title><style>p{color:red}</style>"));
}

#[test]
fn str_replace_on_a_scripted_page_is_refused_and_keeps_the_scripts_out_of_reach() {
    stripped(applied(SCRIPTED, &[str_replace("Second para", "Swapped")]));
}

#[test]
fn str_replace_on_a_sample_page_with_an_id_is_refused_rather_than_stripping_it() {
    stripped(applied(
        SAMPLE_HTML,
        &[str_replace("Second para", "Swapped")],
    ));
}

#[test]
fn str_replace_on_a_classed_page_lands_and_keeps_its_classes() {
    let page = b"<!DOCTYPE html><html><head><style>.lead{color:red}</style></head>\
<body class=\"page\"><h1 class=\"title main\">Heading</h1><p class=\"lead\">First para</p>\
<ul class=\"items\"><li class=\"item\">one</li></ul><p>Second para</p></body></html>";
    let out = applied(page, &[str_replace("Second para", "Swapped")]).expect("classes are kept");
    assert!(
        out.contains(r#"<h1 class="title main">Heading</h1>"#),
        "{out}"
    );
    assert!(out.contains(r#"<p class="lead">First para</p>"#), "{out}");
    assert!(out.contains(r#"<li class="item">one</li>"#), "{out}");
    assert!(out.contains("<p>Swapped</p>"), "{out}");
}

#[test]
fn str_replace_on_a_page_with_an_id_or_inline_style_is_refused() {
    for body in [
        r#"<h1 id="top">Heading</h1><p>Second para</p>"#,
        r#"<h1 style="color:red">Heading</h1><p>Second para</p>"#,
        "<style>p{color:red}</style><h1>Heading</h1><p>Second para</p>",
    ] {
        let page = format!("<!DOCTYPE html><html><head></head><body>{body}</body></html>");
        stripped(applied(
            page.as_bytes(),
            &[str_replace("Second para", "Swapped")],
        ));
    }
}

#[test]
fn str_replace_on_a_plain_page_lands_and_keeps_the_head_script() {
    let out = applied(PLAIN, &[str_replace("Second para", "Swapped")]).expect("plain page");
    assert!(out.contains("<p>Swapped</p>"), "{out}");
    assert!(
        out.contains(r#"<a href="https://example.com/">link</a>"#),
        "{out}"
    );
    assert!(!out.contains("noopener"), "{out}");
    assert!(out.contains("<script>boot()</script>"), "{out}");
}

#[test]
fn str_replace_introducing_a_script_is_refused() {
    stripped(applied(
        PLAIN,
        &[str_replace("Second para", "<script>alert(1)</script>")],
    ));
}

#[test]
fn a_pending_block_survives_str_replace() {
    let skeleton = applied(
        PLAIN,
        &[EditOp::AppendMarkdown {
            markdown: "<!-- pending: Goals: three outcomes -->\nSee [the plan](https://example.com/plan).\n"
                .to_owned(),
        }],
    )
    .expect("append");
    assert!(skeleton.contains("data-pending"), "{skeleton}");
    let out = applied(
        skeleton.as_bytes(),
        &[str_replace("Second para", "Swapped")],
    )
    .expect("the codec's own markup passes its sanitiser");
    assert!(
        out.contains(r#"data-pending="Goals: three outcomes""#),
        "{out}"
    );
    assert!(out.contains("<p>Swapped</p>"), "{out}");
    let projection = HtmlCodec.project(out.as_bytes()).expect("reproject");
    assert!(
        projection
            .nodes()
            .any(|node| node.kind == NodeKind::Pending),
        "{out}"
    );
}

#[test]
fn plain_text_and_instantiate() {
    let text = HtmlCodec.plain_text(SAMPLE_HTML).expect("text");
    assert!(text.starts_with("Heading\nFirst bold para\nSecond para\none\ntwo"));
    let mut params = TemplateParams::default();
    params.values.insert("who".into(), "<Ada>".into());
    let out = HtmlCodec
        .instantiate(b"<html><body><p>Hi {{who}}</p></body></html>", &params)
        .expect("instantiate");
    assert!(
        String::from_utf8(out)
            .expect("utf8")
            .contains("<p>Hi &lt;Ada&gt;</p>")
    );
}
