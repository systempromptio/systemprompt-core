use systemprompt_files::documents::codecs::HtmlCodec;
use systemprompt_provider_contracts::documents::{DocumentCodec, EditOp, NodeKind, TemplateParams};

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

#[test]
fn set_html_is_sanitised_and_head_kept() {
    let base = HtmlCodec.project(SAMPLE_HTML).expect("project");
    let op = EditOp::SetHtml {
        html: r#"<p onclick="x()">Safe</p><script>alert(1)</script>"#.into(),
    };
    let out = String::from_utf8(HtmlCodec.apply(SAMPLE_HTML, &base, &[op]).expect("apply"))
        .expect("utf8");
    assert!(out.contains("<p>Safe</p>"));
    assert!(!out.contains("alert"));
    assert!(!out.contains("onclick"));
    assert!(out.contains("<title>Keep Me</title>"));
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
