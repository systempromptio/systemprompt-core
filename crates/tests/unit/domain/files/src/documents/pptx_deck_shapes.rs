use systemprompt_files::documents::codecs::pptx::{deck_model, lower_deck};
use systemprompt_files::documents::engine::loro::deck::{DeckShape, Frame, ShapeKind};

use super::pptx_deck_lower::base;
use super::pptx_deck_model::{FRAMED, shape};
use super::pptx_deck_structure::{FRAME, added};
use super::pptx_fixture::part;

#[test]
fn an_added_image_links_its_picture_part_and_bad_images_are_skipped() {
    let (bytes, mut deck) = base();
    let picture = |id: &str, src: Option<&str>, frame: Option<Frame>| DeckShape {
        src: src.map(str::to_owned),
        ..added(id, ShapeKind::Image, frame, Vec::new())
    };
    let slide = deck
        .slides
        .iter_mut()
        .find(|s| s.id == FRAMED)
        .expect("slide");
    slide
        .shapes
        .push(picture("p1", Some("ppt/media/image2.png"), Some(FRAME)));
    slide
        .shapes
        .push(picture("p2", Some("ppt/media/image1.png"), Some(FRAME)));
    slide
        .shapes
        .push(picture("p3", Some("ppt/media/missing.png"), Some(FRAME)));
    slide.shapes.push(picture("p4", None, Some(FRAME)));
    slide
        .shapes
        .push(picture("p5", Some("ppt/media/image2.png"), None));
    let out = lower_deck(&bytes, &deck).expect("lower");
    let back = deck_model(&out).expect("deck");
    assert_eq!(
        back.slides
            .iter()
            .find(|s| s.id == FRAMED)
            .expect("slide")
            .shapes
            .len(),
        7
    );
    let first = shape(&back, FRAMED, "7");
    assert_eq!(first.kind, ShapeKind::Image);
    assert_eq!(first.src.as_deref(), Some("ppt/media/image2.png"));
    assert_eq!(first.frame, Some(FRAME));
    assert_eq!(
        shape(&back, FRAMED, "8").src.as_deref(),
        Some("ppt/media/image1.png")
    );
    let rels = part(&out, "ppt/slides/_rels/slide1.xml.rels");
    assert_eq!(rels.matches("Target=\"../media/image2.png\"").count(), 1);
    assert_eq!(rels.matches("Target=\"../media/image1.png\"").count(), 1);
}

#[test]
fn an_image_on_a_slide_without_relationships_gets_a_new_rels_part() {
    let (bytes, mut deck) = base();
    let slide = deck
        .slides
        .iter_mut()
        .find(|s| s.id != FRAMED)
        .expect("slide");
    let id = slide.id.clone();
    slide.shapes.push(DeckShape {
        src: Some("ppt/media/image2.png".to_owned()),
        ..added("p", ShapeKind::Image, Some(FRAME), Vec::new())
    });
    let out = lower_deck(&bytes, &deck).expect("lower");
    let back = deck_model(&out).expect("deck");
    let read = back.slides.iter().find(|s| s.id == id).expect("slide");
    assert_eq!(
        read.shapes.last().and_then(|s| s.src.as_deref()),
        Some("ppt/media/image2.png")
    );
}

#[test]
fn removed_shapes_are_cut_and_a_new_z_order_rewrites_the_tree() {
    let (bytes, mut deck) = base();
    let slide = deck
        .slides
        .iter_mut()
        .find(|s| s.id == FRAMED)
        .expect("slide");
    slide.shapes.retain(|s| s.id != "shape-3");
    slide.shapes.reverse();
    let out = lower_deck(&bytes, &deck).expect("lower");
    let back = deck_model(&out).expect("deck");
    let ids: Vec<&str> = back
        .slides
        .iter()
        .find(|s| s.id == FRAMED)
        .expect("slide")
        .shapes
        .iter()
        .map(|s| s.id.as_str())
        .collect();
    assert_eq!(ids, vec!["6", "4", "3", "2"]);
    assert!(!part(&out, FRAMED).contains("Twin"));
}

#[test]
fn a_removed_shape_alone_is_spliced_out_in_place() {
    let (bytes, mut deck) = base();
    let slide = deck
        .slides
        .iter_mut()
        .find(|s| s.id == FRAMED)
        .expect("slide");
    slide.shapes.retain(|s| s.id != "4");
    let out = lower_deck(&bytes, &deck).expect("lower");
    let xml = part(&out, FRAMED);
    assert!(!xml.contains(">Note<"));
    assert!(xml.contains(">Agenda<") && xml.contains(">Twin<"));
    let back = deck_model(&out).expect("deck");
    assert_eq!(shape(&back, FRAMED, "4").paragraphs[0].text, "Twin");
}
