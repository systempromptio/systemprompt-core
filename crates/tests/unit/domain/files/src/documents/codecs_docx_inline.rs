use super::docx_fixture::{package_with, part, project, replace, text_of};

const HEAD: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" mc:Ignorable="w14"><w:body>"#,
);
const TAIL: &str = "</w:body></w:document>";

fn doc(paragraphs: &str) -> Vec<u8> {
    package_with(&format!("{HEAD}{paragraphs}{TAIL}"))
}

#[test]
fn inline_containers_contribute_their_runs() {
    let bytes = doc(concat!(
        r#"<w:p w14:paraId="00000001"><w:pPr/>"#,
        r#"<w:hyperlink r:id="rId1"><w:ins w:id="1"><w:r><w:t>linked</w:t></w:r></w:ins><w:other><w:x/></w:other></w:hyperlink>"#,
        r#"<w:r><w:t xml:space="preserve"> text</w:t></w:r>"#,
        "</w:p>",
    ));
    let projection = project(&bytes);
    assert_eq!(text_of(&projection, "pid:00000001"), "linked text");
}

#[test]
fn run_children_map_to_text_and_toggles() {
    let bytes = doc(concat!(
        r#"<w:p w14:paraId="00000002">"#,
        r#"<w:r><w:rPr><w:b w:val="0"></w:b><w:i/></w:rPr><w:t>a</w:t><w:noBreakHyphen/><w:t>b</w:t><w:unknownEmpty/><w:extra><w:y/></w:extra></w:r>"#,
        r#"<w:r><w:t><![CDATA[c]]>&#68;&amp;<w:nested/></w:t></w:r>"#,
        r#"<w:r/>"#,
        r#"<w:customBlock><w:z/></w:customBlock>"#,
        "</w:p>",
    ));
    let projection = project(&bytes);
    assert_eq!(text_of(&projection, "pid:00000002"), "a-bcD&");
}

#[test]
fn replacing_text_keeps_object_runs_inside_containers() {
    let bytes = doc(concat!(
        r#"<w:p w14:paraId="00000003">"#,
        r#"<w:sdt><w:sdtContent><w:r><w:drawing><w:inner/></w:drawing></w:r></w:sdtContent></w:sdt>"#,
        r#"<w:r><w:t>old caption</w:t></w:r>"#,
        "</w:p>",
    ));
    let out = replace(&bytes, "pid:00000003", "new caption");
    let xml = part(&out, "word/document.xml");
    assert!(xml.contains("<w:drawing><w:inner/></w:drawing>"), "{xml}");
    assert!(xml.contains("new caption"));
    assert!(!xml.contains("old caption"));
}

#[test]
fn ppr_with_nested_children_keeps_style() {
    let bytes = doc(concat!(
        r#"<w:p w14:paraId="00000004"><w:pPr><w:pStyle w:val="Heading2"/><w:spacing><w:inner/></w:spacing><!-- note --></w:pPr>"#,
        r#"<w:r><w:rPr><w:u><w:x/></w:u><!-- c --></w:rPr><w:t>Title</w:t></w:r></w:p>"#,
    ));
    let projection = project(&bytes);
    let node = projection
        .root
        .iter()
        .find(|n| n.id.as_str() == "pid:00000004")
        .expect("para");
    assert_eq!(node.text.as_deref(), Some("Title"));
    assert!(matches!(
        node.kind,
        systemprompt_provider_contracts::documents::NodeKind::Heading { level: 2 }
    ));
}
