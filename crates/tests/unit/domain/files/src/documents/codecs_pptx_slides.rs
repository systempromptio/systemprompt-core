use systemprompt_files::documents::codecs::pptx::{PptxCodec, blank_pptx};
use systemprompt_provider_contracts::documents::{DocumentCodec, EditOp, NodeTarget};

use super::pptx_fixture::{apply, hash_of, node, part, project, rebuild};

fn insert_slide(after: Option<&str>, title: &str) -> EditOp {
    EditOp::InsertSlide {
        after: after.map(node),
        layout: None,
        title: Some(title.to_owned()),
        body: Vec::new(),
    }
}

fn without_slide_list() -> Vec<u8> {
    let blank = blank_pptx().expect("blank");
    let presentation = part(&blank, "ppt/presentation.xml").replace(
        "<p:sldIdLst><p:sldId id=\"256\" r:id=\"rId2\"/></p:sldIdLst>",
        "",
    );
    let app = part(&blank, "docProps/app.xml").replace("<Slides>1</Slides>", "<Slides/>");
    rebuild(
        &blank,
        &[
            ("ppt/presentation.xml", presentation),
            ("docProps/app.xml", app),
        ],
    )
}

#[test]
fn a_deck_without_a_slide_list_projects_no_slides() {
    assert!(project(&without_slide_list()).root.is_empty());
}

#[test]
fn inserting_into_a_deck_without_a_slide_list_creates_one() {
    let edited = apply(&without_slide_list(), &[insert_slide(None, "First")]);
    let presentation = part(&edited, "ppt/presentation.xml");
    let masters = presentation.find("</p:sldMasterIdLst>").expect("masters");
    let list = presentation.find("<p:sldIdLst><p:sldId").expect("list");
    assert!(masters < list, "{presentation}");
    assert!(part(&edited, "docProps/app.xml").contains("<Slides>1</Slides>"));
    let projection = project(&edited);
    assert_eq!(projection.root.len(), 1);
    assert_eq!(projection.root[0].text.as_deref(), Some("First"));
}

#[test]
fn a_deck_without_masters_refuses_a_new_slide_list() {
    let blank = blank_pptx().expect("blank");
    let presentation = part(&blank, "ppt/presentation.xml")
        .replace(
            "<p:sldIdLst><p:sldId id=\"256\" r:id=\"rId2\"/></p:sldIdLst>",
            "",
        )
        .replace(
            "<p:sldMasterIdLst><p:sldMasterId id=\"2147483648\" r:id=\"rId1\"/></p:sldMasterIdLst>",
            "",
        );
    let deck = rebuild(&blank, &[("ppt/presentation.xml", presentation)]);
    let base = project(&deck);
    let result = PptxCodec.apply(&deck, &base, &[insert_slide(None, "x")]);
    assert!(result.is_err());
}

#[test]
fn a_deck_without_app_properties_still_deletes_slides() {
    let blank = blank_pptx().expect("blank");
    let app = part(&blank, "docProps/app.xml").replace("<Slides>1</Slides>", "");
    let deck = rebuild(&blank, &[("docProps/app.xml", app)]);
    let added = apply(&deck, &[insert_slide(Some("slide[1]"), "Second")]);
    let hash = hash_of(&project(&added), "slide[1]");
    let edited = apply(
        &added,
        &[EditOp::Delete {
            node: NodeTarget::Id(node("slide[1]")),
            expected_hash: Some(hash),
        }],
    );
    let projection = project(&edited);
    assert_eq!(projection.root.len(), 1);
    assert_eq!(projection.root[0].text.as_deref(), Some("Second"));
    assert!(!part(&edited, "docProps/app.xml").contains("<Slides>"));
}
