use systemprompt_files::documents::codecs::pptx::{blank_pptx, deck_model, lower_deck};
use systemprompt_files::documents::engine::loro::deck::{DeckModel, DeckShape, Frame, ShapeKind};
use systemprompt_files::documents::model::{BlockKind, Mark, MarkKind};
use systemprompt_provider_contracts::documents::CodecError;

use super::pptx_fixture::{framed_deck, two_slide_deck};

pub(super) const FRAMED: &str = "ppt/slides/slide1.xml";

pub(super) fn shape<'d>(deck: &'d DeckModel, slide: &str, id: &str) -> &'d DeckShape {
    deck.slides
        .iter()
        .find(|s| s.id == slide)
        .and_then(|s| s.shapes.iter().find(|sh| sh.id == id))
        .expect("shape")
}

fn mark(start: usize, end: usize, kind: MarkKind) -> Mark {
    Mark { start, end, kind }
}

fn sorted(marks: &[Mark]) -> Vec<(usize, usize, &'static str)> {
    let mut out: Vec<_> = marks
        .iter()
        .map(|m| (m.start, m.end, m.kind.key()))
        .collect();
    out.sort_unstable();
    out
}

#[test]
fn slides_follow_the_presentation_order_with_their_part_names() {
    let deck = deck_model(&framed_deck()).expect("deck");
    let ids: Vec<&str> = deck.slides.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, vec!["ppt/slides/slide2.xml", FRAMED]);
    assert!(deck.slides.iter().all(|s| s.layout.is_some()));
}

#[test]
fn shapes_carry_their_ids_kinds_and_frames() {
    let deck = deck_model(&framed_deck()).expect("deck");
    let slide = deck.slides.iter().find(|s| s.id == FRAMED).expect("slide");
    let read: Vec<(&str, ShapeKind)> = slide
        .shapes
        .iter()
        .map(|s| (s.id.as_str(), s.kind))
        .collect();
    assert_eq!(
        read,
        vec![
            ("2", ShapeKind::Title),
            ("3", ShapeKind::Body),
            ("4", ShapeKind::Text),
            ("shape-3", ShapeKind::Text),
            ("6", ShapeKind::Image),
        ]
    );
    assert_eq!(
        shape(&deck, FRAMED, "2").frame,
        Some(Frame {
            x: 100,
            y: 200,
            w: 3000,
            h: 400
        })
    );
    assert_eq!(shape(&deck, FRAMED, "3").frame, None);
    assert_eq!(shape(&deck, FRAMED, "shape-3").frame, None);
    let picture = shape(&deck, FRAMED, "6");
    assert_eq!(
        picture.frame,
        Some(Frame {
            x: 10,
            y: 20,
            w: 30,
            h: 40
        })
    );
    assert_eq!(picture.src.as_deref(), Some("ppt/media/image1.png"));
    assert!(picture.paragraphs.is_empty());
}

#[test]
fn run_properties_read_as_merged_marks() {
    let deck = deck_model(&framed_deck()).expect("deck");
    let body = shape(&deck, FRAMED, "3");
    assert_eq!(body.paragraphs[0].text, "Bold plain");
    assert_eq!(body.paragraphs[0].id, format!("{FRAMED}#3/p0"));
    let first = &body.paragraphs[0].marks;
    for expected in [
        mark(0, 4, MarkKind::Bold),
        mark(0, 4, MarkKind::Underline),
        mark(0, 4, MarkKind::Strike),
        mark(0, 4, MarkKind::Caps),
        mark(
            0,
            4,
            MarkKind::Size {
                value: "24".to_owned(),
            },
        ),
        mark(
            0,
            4,
            MarkKind::Color {
                value: "#1F4E79".to_owned(),
            },
        ),
        mark(
            0,
            4,
            MarkKind::Font {
                value: "Georgia".to_owned(),
            },
        ),
    ] {
        assert!(first.contains(&expected), "missing {expected:?}");
    }
    assert_eq!(first.len(), 7);
    assert_eq!(
        sorted(&body.paragraphs[1].marks),
        vec![
            (1, 2, "highlight"),
            (1, 2, "superscript"),
            (2, 3, "subscript")
        ]
    );
    let note = &shape(&deck, FRAMED, "4").paragraphs[0];
    assert_eq!(
        note.marks,
        vec![
            mark(0, 4, MarkKind::Italic),
            mark(
                0,
                4,
                MarkKind::Size {
                    value: "10.5".to_owned()
                }
            ),
        ]
    );
}

#[test]
fn a_line_break_counts_as_one_character_and_bullets_become_list_items() {
    let deck = deck_model(&framed_deck()).expect("deck");
    let body = shape(&deck, FRAMED, "3");
    assert_eq!(body.paragraphs[2].text, "Hiring\nplan");
    assert!(
        body.paragraphs
            .iter()
            .all(|p| matches!(p.kind, BlockKind::ListItem { .. }))
    );
    let twin = shape(&deck, FRAMED, "shape-3");
    assert_eq!(twin.paragraphs[0].kind, BlockKind::Paragraph);
    assert_eq!(
        shape(&deck, FRAMED, "4").paragraphs[0].kind,
        BlockKind::Paragraph
    );
}

#[test]
fn tables_read_as_table_shapes_without_paragraphs() {
    let deck = deck_model(&two_slide_deck()).expect("deck");
    let table = shape(&deck, FRAMED, "5");
    assert_eq!(table.kind, ShapeKind::Table);
    assert!(table.paragraphs.is_empty());
    assert!(shape(&deck, FRAMED, "6").src.is_some());
}

#[test]
fn a_blank_deck_reads_as_one_slide() {
    let deck = deck_model(&blank_pptx().expect("blank")).expect("deck");
    assert_eq!(deck.slides.len(), 1);
}

#[test]
fn bytes_that_are_not_a_package_are_refused() {
    let refused: Result<DeckModel, CodecError> = deck_model(b"nope");
    assert!(refused.is_err());
    assert!(lower_deck(b"nope", &DeckModel::default()).is_err());
}
