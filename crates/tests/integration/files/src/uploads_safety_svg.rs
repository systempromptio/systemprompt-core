use systemprompt_files::uploads::RefusalReason;

use crate::uploads_rig::{UploadRig, refusal_detail, rig};

const SVG: &str = "image/svg+xml";

async fn refused(rig: &UploadRig, body: &str) -> String {
    let (reason, detail) = refusal_detail(rig.put(SVG, body.as_bytes()).await);
    assert_eq!(reason, RefusalReason::UnsafeSvg, "{detail}");
    detail
}

async fn passes(rig: &UploadRig, body: &str) {
    let completed = rig
        .put(SVG, body.as_bytes())
        .await
        .unwrap_or_else(|error| panic!("{body}: {error:?}"));
    assert_eq!(completed.checked_kind, "svg");
}

fn svg(inner: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">{inner}</svg>"#
    )
}

#[tokio::test]
async fn inert_metadata_fragments_styles_and_inline_pictures_pass() {
    let rig = rig().await;
    passes(
        &rig,
        &format!(
            "\u{feff}<?xml version=\"1.0\"?><!-- drawn --><!DOCTYPE svg>{}",
            svg(concat!(
                r#"<defs><linearGradient id="g"/></defs>"#,
                r#"<style>rect { fill: url(#g) } /* &#x41; &amp; */</style>"#,
                r##"<use xlink:href="#g"/>"##,
                r#"<image href="data:image/png;base64,AAAA"/>"#,
                r#"<rect style="fill: url('#g')" xml:lang="en"/>"#,
                r#"<text>caf&#233;</text>"#,
            ))
        ),
    )
    .await;
    passes(
        &rig,
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" inkscape:version="1"><inkscape:grid/><style><![CDATA[rect{}]]></style></svg>"#,
    )
    .await;
}

#[tokio::test]
async fn external_references_and_stylesheet_constructs_are_refused() {
    let rig = rig().await;
    let cases = [
        (
            r#"<use xlink:href="https://evil.test/x.svg#a"/>"#,
            "not a #fragment",
        ),
        (r#"<image href="data:text/html,hi"/>"#, "not a #fragment"),
        (
            r#"<a href="javascript:alert(1)"><rect/></a>"#,
            "not a #fragment",
        ),
        (
            r#"<style>@import url(x.css);</style>"#,
            "stylesheet construct",
        ),
        (
            r#"<rect style="fill: url(https://x/y)"/>"#,
            "stylesheet url()",
        ),
        (
            r#"<style>rect{}<rect/></style>"#,
            "style element holds markup",
        ),
        (
            r#"<style>rect{}<g></g></style>"#,
            "style element holds markup",
        ),
        (r#"<foreignObject/>"#, "a <foreignObject> element"),
        (
            r#"<rect xml:base="https://x/"/>"#,
            "of the http://www.w3.org/XML/1998/namespace namespace",
        ),
    ];
    for (inner, needle) in cases {
        let detail = refused(&rig, &svg(inner)).await;
        assert!(detail.contains(needle), "{inner}: {detail}");
    }
}

#[tokio::test]
async fn foreign_namespaces_prefixes_dtds_and_instructions_are_refused() {
    let rig = rig().await;
    let cases = [
        (
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:h="http://www.w3.org/1999/xhtml"><h:div/></svg>"#,
            "of the http://www.w3.org/1999/xhtml namespace",
        ),
        (
            r#"<svg xmlns="http://www.w3.org/2000/svg"><q:thing/></svg>"#,
            "unbound prefix",
        ),
        (
            r#"<svg xmlns="http://www.w3.org/2000/svg"><rect q:fill="red"/></svg>"#,
            "unbound prefix",
        ),
        (
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:e="urn:evil"><rect e:x="1"/></svg>"#,
            "of the urn:evil namespace",
        ),
        (
            r#"<!DOCTYPE svg [<!ENTITY a "b">]><svg xmlns="http://www.w3.org/2000/svg"/>"#,
            "internal DTD subset",
        ),
        (
            r#"<svg xmlns="http://www.w3.org/2000/svg"><?php echo 1; ?></svg>"#,
            "processing instruction",
        ),
        (
            "<!-- <svg xmlns=\"http://www.w3.org/2000/svg\"/> -->",
            "holds no element",
        ),
    ];
    for (body, needle) in cases {
        let detail = refused(&rig, body).await;
        assert!(detail.contains(needle), "{body}: {detail}");
    }
}

#[tokio::test]
async fn malformed_attributes_and_references_cannot_be_read() {
    let rig = rig().await;
    let detail = refused(&rig, &svg(r#"<rect width="1" width="2"/>"#)).await;
    assert!(detail.contains("cannot be read"), "{detail}");
    let detail = refused(&rig, &svg("<text>&#xD800;</text>")).await;
    assert!(detail.contains("cannot be read"), "{detail}");
}
