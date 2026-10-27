use systemprompt_files::documents::codecs::pptx::{PptxCodec, blank_pptx};
use systemprompt_provider_contracts::documents::{CodecError, DocumentCodec, EditOp, NodeKind};

use super::pptx_fixture::{
    apply, entries, hash_of, node, part, project, raw_entries, two_slide_deck,
};

fn replace(bytes: &[u8], id: &str, text: &str) -> Vec<u8> {
    let expected_hash = hash_of(&project(bytes), id);
    apply(
        bytes,
        &[EditOp::ReplaceText {
            node: node(id),
            expected_hash,
            text: text.to_owned(),
        }],
    )
}

#[test]
fn replace_text_keeps_the_first_run_properties() {
    let edited = replace(
        &two_slide_deck(),
        "slide[2]/shape[1]/p[0]",
        "Profit\nand loss",
    );
    let xml = part(&edited, "ppt/slides/slide1.xml");
    assert!(xml.contains(
        "<a:p><a:r><a:rPr lang=\"en-US\" i=\"1\"/><a:t>Profit</a:t></a:r><a:br><a:rPr lang=\"en-US\" i=\"1\"/></a:br><a:r><a:rPr lang=\"en-US\" i=\"1\"/><a:t>and loss</a:t></a:r></a:p>"
    ));
    let text = project(&edited)
        .find(&node("slide[2]/shape[1]/p[0]"))
        .and_then(|p| p.text.clone());
    assert_eq!(text.as_deref(), Some("Profit\nand loss"));
}

#[test]
fn replace_text_rewrites_only_the_edited_slide_part() {
    let deck = two_slide_deck();
    let edited = replace(&deck, "slide[2]/shape[1]/p[1]", "Hiring <fast>");
    let before = raw_entries(&deck);
    let after = raw_entries(&edited);
    let names = |list: &[(String, Vec<u8>)]| {
        list.iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(&before), names(&after));
    let changed: Vec<&str> = before
        .iter()
        .zip(&after)
        .filter(|(old, new)| old.1 != new.1)
        .map(|(old, _)| old.0.as_str())
        .collect();
    assert_eq!(changed, ["ppt/slides/slide1.xml"]);
    let old_xml = part(&deck, "ppt/slides/slide1.xml");
    let new_xml = part(&edited, "ppt/slides/slide1.xml");
    assert!(new_xml.starts_with(&old_xml[..old_xml.find("Content 2").expect("shape")]));
    assert!(new_xml.contains(
        "<a:pPr lvl=\"1\"/><a:r><a:rPr lang=\"en-US\"/><a:t>Hiring &lt;fast&gt;</a:t></a:r>"
    ));
}

#[test]
fn empty_text_leaves_only_the_end_paragraph_properties() {
    let edited = replace(&blank_pptx().expect("blank"), "slide[1]/shape[0]/p[0]", "");
    assert!(
        part(&edited, "ppt/slides/slide1.xml")
            .contains("<a:p><a:endParaRPr lang=\"en-US\"/></a:p>")
    );
}

#[test]
fn stale_hash_reports_the_current_text() {
    let deck = two_slide_deck();
    let err = PptxCodec
        .apply(
            &deck,
            &project(&deck),
            &[EditOp::ReplaceText {
                node: node("slide[2]/shape[1]/p[0]"),
                expected_hash: "00000000".to_owned(),
                text: "x".to_owned(),
            }],
        )
        .expect_err("stale");
    match err {
        CodecError::StaleAnchor { current_text, .. } => assert_eq!(current_text, "Revenue & costs"),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn insert_after_as_paragraph_switches_bullets_off() {
    let deck = two_slide_deck();
    let edited = apply(
        &deck,
        &[EditOp::InsertAfter {
            node: node("slide[2]/shape[1]/p[0]"),
            kind: NodeKind::Paragraph,
            text: "Note".to_owned(),
        }],
    );
    let projection = project(&edited);
    let inserted = projection
        .find(&node("slide[2]/shape[1]/p[1]"))
        .expect("inserted");
    assert_eq!(inserted.text.as_deref(), Some("Note"));
    assert!(!inserted.attrs.contains_key("bullet"));
    assert_eq!(
        projection
            .find(&node("slide[2]/shape[1]/p[2]"))
            .and_then(|p| p.text.clone())
            .as_deref(),
        Some("Hiring\nplan")
    );
}

#[test]
fn insert_after_as_list_item_keeps_the_paragraph_properties() {
    let edited = apply(
        &two_slide_deck(),
        &[EditOp::InsertAfter {
            node: node("slide[2]/shape[1]/p[1]"),
            kind: NodeKind::ListItem,
            text: "Training".to_owned(),
        }],
    );
    assert!(part(&edited, "ppt/slides/slide1.xml").contains(
        "<a:p><a:pPr lvl=\"1\"/><a:r><a:rPr lang=\"en-US\"/><a:t>Training</a:t></a:r></a:p>"
    ));
}

#[test]
fn deleting_the_last_paragraph_keeps_an_empty_one() {
    let deck = two_slide_deck();
    let expected_hash = hash_of(&project(&deck), "slide[1]/shape[0]/p[0]");
    let edited = apply(
        &deck,
        &[EditOp::Delete {
            node: node("slide[1]/shape[0]/p[0]"),
            expected_hash,
        }],
    );
    let projection = project(&edited);
    assert_eq!(
        projection
            .find(&node("slide[1]/shape[0]/p[0]"))
            .and_then(|p| p.text.clone())
            .as_deref(),
        Some("")
    );
}

#[test]
fn delete_slide_removes_it_from_the_order() {
    let deck = two_slide_deck();
    let expected_hash = hash_of(&project(&deck), "slide[1]");
    let edited = apply(
        &deck,
        &[EditOp::Delete {
            node: node("slide[1]"),
            expected_hash,
        }],
    );
    let projection = project(&edited);
    assert_eq!(projection.root.len(), 1);
    assert_eq!(projection.root[0].text.as_deref(), Some("Agenda"));
    assert!(!part(&edited, "ppt/presentation.xml").contains("rId7"));
    assert!(!part(&edited, "ppt/_rels/presentation.xml.rels").contains("rId7"));
    assert!(part(&edited, "docProps/app.xml").contains("<Slides>1</Slides>"));
}

#[test]
fn insert_slide_adds_parts_ids_and_relationships() {
    let deck = two_slide_deck();
    let edited = apply(
        &deck,
        &[EditOp::InsertSlide {
            after: Some(node("slide[1]")),
            layout: None,
            title: Some("Roadmap".to_owned()),
            body: vec!["Ship".to_owned(), "Measure".to_owned()],
        }],
    );
    let before: Vec<String> = entries(&deck).into_iter().map(|(name, _)| name).collect();
    let after: Vec<String> = entries(&edited).into_iter().map(|(name, _)| name).collect();
    assert_eq!(&after[..before.len()], before.as_slice());
    assert_eq!(
        &after[before.len()..],
        ["ppt/slides/slide3.xml", "ppt/slides/_rels/slide3.xml.rels"]
    );

    let presentation = part(&edited, "ppt/presentation.xml");
    assert!(presentation.contains("<p:sldId id=\"300\" r:id=\"rId7\"/><p:sldId id=\"301\" r:id=\"rId8\"/><p:sldId id=\"256\" r:id=\"rId2\"/>"));
    assert!(part(&edited, "ppt/_rels/presentation.xml.rels").contains("<Relationship Id=\"rId8\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide\" Target=\"slides/slide3.xml\"/>"));
    assert!(
        part(&edited, "[Content_Types].xml")
            .contains("<Override PartName=\"/ppt/slides/slide3.xml\"")
    );
    assert!(
        part(&edited, "ppt/slides/_rels/slide3.xml.rels")
            .contains("Target=\"../slideLayouts/slideLayout1.xml\"")
    );
    assert!(part(&edited, "docProps/app.xml").contains("<Slides>3</Slides>"));

    let projection = project(&edited);
    let inserted = &projection.root[1];
    assert_eq!(inserted.text.as_deref(), Some("Roadmap"));
    assert_eq!(
        inserted.attrs.get("part").map(String::as_str),
        Some("ppt/slides/slide3.xml")
    );
    let bullets: Vec<(Option<String>, bool)> = inserted.children[1]
        .children
        .iter()
        .map(|p| (p.text.clone(), p.attrs.contains_key("bullet")))
        .collect();
    assert_eq!(
        bullets,
        [
            (Some("Ship".to_owned()), true),
            (Some("Measure".to_owned()), true)
        ]
    );
    assert_eq!(projection.root[2].text.as_deref(), Some("Agenda"));
}

#[test]
fn insert_slide_with_an_unknown_layout_is_rejected() {
    let deck = blank_pptx().expect("blank");
    let err = PptxCodec
        .apply(
            &deck,
            &project(&deck),
            &[EditOp::InsertSlide {
                after: None,
                layout: Some("Nope".to_owned()),
                title: None,
                body: Vec::new(),
            }],
        )
        .expect_err("unknown layout");
    assert!(matches!(err, CodecError::Malformed { .. }));
}

#[test]
fn str_replace_requires_one_match() {
    let deck = two_slide_deck();
    let edited = apply(
        &deck,
        &[EditOp::StrReplace {
            old_str: "costs".to_owned(),
            new_str: "spend".to_owned(),
        }],
    );
    let text = project(&edited)
        .find(&node("slide[2]/shape[1]/p[0]"))
        .and_then(|p| p.text.clone());
    assert_eq!(text.as_deref(), Some("Revenue & spend"));
    let ambiguous = PptxCodec
        .apply(
            &deck,
            &project(&deck),
            &[EditOp::StrReplace {
                old_str: "n".to_owned(),
                new_str: "m".to_owned(),
            }],
        )
        .expect_err("ambiguous");
    assert!(matches!(ambiguous, CodecError::AmbiguousMatch { .. }));
    let missing = PptxCodec
        .apply(
            &deck,
            &project(&deck),
            &[EditOp::StrReplace {
                old_str: "zebra".to_owned(),
                new_str: "m".to_owned(),
            }],
        )
        .expect_err("missing");
    assert!(matches!(missing, CodecError::NoMatch));
}

#[test]
fn whole_document_ops_are_unsupported() {
    let deck = blank_pptx().expect("blank");
    let err = PptxCodec
        .apply(
            &deck,
            &project(&deck),
            &[EditOp::SetText {
                text: "x".to_owned(),
            }],
        )
        .expect_err("unsupported");
    assert!(matches!(
        err,
        CodecError::UnsupportedOp { op: "set_text", .. }
    ));
}
