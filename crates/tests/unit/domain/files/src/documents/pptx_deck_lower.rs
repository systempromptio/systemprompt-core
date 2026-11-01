use systemprompt_files::documents::codecs::pptx::{deck_model, lower_deck};
use systemprompt_files::documents::engine::loro::deck::{
    DeckModel, DeckShape, DeckSlide, ShapeKind,
};
use systemprompt_files::documents::model::{Block, BlockKind};

use super::pptx_deck_model::{FRAMED, shape};
use super::pptx_fixture::{entries, framed_deck, part, project};

pub(super) fn base() -> (Vec<u8>, DeckModel) {
    let bytes = framed_deck();
    let deck = deck_model(&bytes).expect("deck");
    (bytes, deck)
}

pub(super) fn shape_mut<'d>(deck: &'d mut DeckModel, slide: &str, id: &str) -> &'d mut DeckShape {
    deck.slides
        .iter_mut()
        .find(|s| s.id == slide)
        .and_then(|s| s.shapes.iter_mut().find(|sh| sh.id == id))
        .expect("shape")
}

pub(super) fn texts(shape: &DeckShape) -> Vec<&str> {
    shape.paragraphs.iter().map(|p| p.text.as_str()).collect()
}

fn para(text: &str) -> Block {
    Block::new("fresh", BlockKind::Paragraph, text)
}

#[test]
fn an_unchanged_model_leaves_the_deck_byte_identical() {
    let (bytes, deck) = base();
    assert_eq!(lower_deck(&bytes, &deck).expect("lower"), bytes);
}

#[test]
fn edited_text_is_replaced_and_the_untouched_marks_survive() {
    let (bytes, mut deck) = base();
    shape_mut(&mut deck, FRAMED, "2").paragraphs[0].text = "Plan".to_owned();
    let out = lower_deck(&bytes, &deck).expect("lower");
    let back = deck_model(&out).expect("deck");
    assert_eq!(texts(shape(&back, FRAMED, "2")), vec!["Plan"]);
    assert_eq!(shape(&back, FRAMED, "3").paragraphs[0].marks.len(), 7);
    assert_eq!(
        part(&out, "ppt/slides/slide2.xml"),
        part(&bytes, "ppt/slides/slide2.xml")
    );
}

#[test]
fn paragraphs_are_inserted_before_between_and_after_and_removed() {
    let (bytes, mut deck) = base();
    let body = shape_mut(&mut deck, FRAMED, "3");
    body.paragraphs.remove(1);
    body.paragraphs.insert(0, para("Opening"));
    body.paragraphs.insert(2, para("Middle"));
    body.paragraphs.push(para("Closing"));
    let out = lower_deck(&bytes, &deck).expect("lower");
    let back = deck_model(&out).expect("deck");
    assert_eq!(
        texts(shape(&back, FRAMED, "3")),
        vec!["Opening", "Bold plain", "Middle", "Hiring\nplan", "Closing"]
    );
}

#[test]
fn several_leading_paragraphs_land_in_front_of_the_first_kept_one() {
    let (bytes, mut deck) = base();
    let note = shape_mut(&mut deck, FRAMED, "4");
    note.paragraphs.insert(0, para("Two"));
    note.paragraphs.insert(0, para("One"));
    let out = lower_deck(&bytes, &deck).expect("lower");
    let back = deck_model(&out).expect("deck");
    assert_eq!(texts(shape(&back, FRAMED, "4")), vec!["One", "Two", "Note"]);
}

#[test]
fn a_rewritten_paragraph_list_changes_each_in_place() {
    let (bytes, mut deck) = base();
    let body = shape_mut(&mut deck, FRAMED, "3");
    for (at, text) in ["A", "B", "C"].into_iter().enumerate() {
        body.paragraphs[at].text = text.to_owned();
    }
    let out = lower_deck(&bytes, &deck).expect("lower");
    let back = deck_model(&out).expect("deck");
    assert_eq!(texts(shape(&back, FRAMED, "3")), vec!["A", "B", "C"]);
}

#[test]
fn a_dropped_slide_is_deleted_and_a_new_one_is_built_from_its_layout() {
    let (bytes, mut deck) = base();
    let layout = deck.slides[0].layout.clone();
    deck.slides.remove(0);
    let title = DeckShape {
        id: "t".to_owned(),
        kind: ShapeKind::Title,
        frame: None,
        paragraphs: vec![para("Fresh")],
        src: None,
    };
    let body = DeckShape {
        id: "b".to_owned(),
        kind: ShapeKind::Body,
        paragraphs: vec![para("one"), para("two")],
        ..title.clone()
    };
    deck.slides.push(DeckSlide {
        id: "new".to_owned(),
        layout,
        shapes: vec![title, body],
    });
    let out = lower_deck(&bytes, &deck).expect("lower");
    let back = deck_model(&out).expect("deck");
    assert_eq!(back.slides.len(), 2);
    assert_eq!(back.slides[0].id, FRAMED);
    let added = &back.slides[1];
    let kinds: Vec<ShapeKind> = added.shapes.iter().map(|s| s.kind).collect();
    assert!(kinds.contains(&ShapeKind::Title));
    let all: Vec<&str> = added.shapes.iter().flat_map(texts).collect();
    assert!(all.contains(&"Fresh"));
    assert!(all.contains(&"one") && all.contains(&"two"));
    assert!(project(&out).root.len() == 2);
}

#[test]
fn a_reversed_slide_order_rewrites_only_the_slide_list() {
    let (bytes, mut deck) = base();
    deck.slides.reverse();
    let out = lower_deck(&bytes, &deck).expect("lower");
    let back = deck_model(&out).expect("deck");
    let ids: Vec<&str> = back.slides.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, vec![FRAMED, "ppt/slides/slide2.xml"]);
    let changed: Vec<String> = entries(&out)
        .into_iter()
        .zip(entries(&bytes))
        .filter(|(after, before)| after != before)
        .map(|(after, _)| after.0)
        .collect();
    assert_eq!(changed, vec!["ppt/presentation.xml"]);
}

#[test]
fn a_renamed_slide_pairs_by_position_and_keeps_its_part() {
    let (bytes, mut deck) = base();
    deck.slides[1].id = "renamed".to_owned();
    shape_mut(&mut deck, "renamed", "4").paragraphs[0].text = "Edited".to_owned();
    let out = lower_deck(&bytes, &deck).expect("lower");
    let back = deck_model(&out).expect("deck");
    assert_eq!(back.slides.len(), 2);
    assert_eq!(texts(shape(&back, FRAMED, "4")), vec!["Edited"]);
}
