use systemprompt_files::documents::codecs::pptx::{deck_model, lower_deck};
use systemprompt_files::documents::engine::loro::deck::{DeckShape, DeckSlide, ShapeKind};
use systemprompt_files::documents::model::{Block, BlockKind};

use super::pptx_deck_lower::{base, shape_mut, texts};
use super::pptx_deck_model::{FRAMED, shape};

fn para(text: &str) -> Block {
    Block::new("fresh", BlockKind::Paragraph, text)
}

#[test]
fn a_slide_missing_from_the_model_is_deleted() {
    let (bytes, mut deck) = base();
    deck.slides.retain(|slide| slide.id == FRAMED);
    let out = lower_deck(&bytes, &deck).expect("lower");
    let back = deck_model(&out).expect("deck");
    assert_eq!(back.slides.len(), 1);
    let all: Vec<&str> = back.slides[0].shapes.iter().flat_map(texts).collect();
    assert!(all.contains(&"Agenda"), "{all:?}");
    assert!(!all.contains(&"Intro"), "{all:?}");
}

#[test]
fn an_extra_slide_is_inserted_with_its_title_and_body() {
    let (bytes, mut deck) = base();
    let layout = deck.slides[0].layout.clone();
    let title = DeckShape {
        id: "t".to_owned(),
        kind: ShapeKind::Title,
        frame: None,
        paragraphs: vec![para("Appendix")],
        src: None,
    };
    let body = DeckShape {
        id: "b".to_owned(),
        kind: ShapeKind::Body,
        paragraphs: vec![para("alpha"), para("beta")],
        ..title.clone()
    };
    deck.slides.push(DeckSlide {
        id: "extra".to_owned(),
        layout,
        shapes: vec![title, body],
    });
    let out = lower_deck(&bytes, &deck).expect("lower");
    let back = deck_model(&out).expect("deck");
    assert_eq!(back.slides.len(), 3);
    let all: Vec<&str> = back.slides[2].shapes.iter().flat_map(texts).collect();
    assert!(all.contains(&"Appendix"), "{all:?}");
    assert!(all.contains(&"alpha") && all.contains(&"beta"), "{all:?}");
}

#[test]
fn an_untitled_extra_slide_is_inserted_without_a_title() {
    let (bytes, mut deck) = base();
    let layout = deck.slides[0].layout.clone();
    deck.slides.push(DeckSlide {
        id: "untitled".to_owned(),
        layout,
        shapes: vec![DeckShape {
            id: "b".to_owned(),
            kind: ShapeKind::Body,
            frame: None,
            paragraphs: vec![para("only body")],
            src: None,
        }],
    });
    let out = lower_deck(&bytes, &deck).expect("lower");
    let back = deck_model(&out).expect("deck");
    assert_eq!(back.slides.len(), 3);
    let all: Vec<&str> = back.slides[2].shapes.iter().flat_map(texts).collect();
    assert!(all.contains(&"only body"), "{all:?}");
}

#[test]
fn renamed_slides_pair_by_part_order() {
    let (bytes, mut deck) = base();
    deck.slides[0].id = "first-renamed".to_owned();
    deck.slides[1].id = "second-renamed".to_owned();
    shape_mut(&mut deck, "first-renamed", "2").paragraphs[0].text = "Plan".to_owned();
    let out = lower_deck(&bytes, &deck).expect("lower");
    let back = deck_model(&out).expect("deck");
    assert_eq!(back.slides.len(), 2);
    assert_eq!(texts(shape(&back, FRAMED, "2")), vec!["Plan"]);
}

#[test]
fn renamed_text_shapes_pair_by_position() {
    let (bytes, mut deck) = base();
    shape_mut(&mut deck, FRAMED, "2").id = "renamed-title".to_owned();
    shape_mut(&mut deck, FRAMED, "4").id = "renamed-note".to_owned();
    shape_mut(&mut deck, FRAMED, "renamed-note").paragraphs[0].text = "Memo".to_owned();
    let out = lower_deck(&bytes, &deck).expect("lower");
    let back = deck_model(&out).expect("deck");
    assert_eq!(texts(shape(&back, FRAMED, "2")), vec!["Agenda"]);
    assert_eq!(texts(shape(&back, FRAMED, "4")), vec!["Memo"]);
}

#[test]
fn a_leading_insert_before_a_changed_paragraph_rewrites_in_front() {
    let (bytes, mut deck) = base();
    let body = shape_mut(&mut deck, FRAMED, "3");
    body.paragraphs[0].text = "Bold plain edited".to_owned();
    body.paragraphs.insert(0, para("Lead"));
    let out = lower_deck(&bytes, &deck).expect("lower");
    let back = deck_model(&out).expect("deck");
    let got = texts(shape(&back, FRAMED, "3"));
    assert_eq!(got[0], "Lead");
    assert!(got.contains(&"Bold plain edited"), "{got:?}");
}
