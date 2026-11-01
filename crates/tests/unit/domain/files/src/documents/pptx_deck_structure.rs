use systemprompt_files::documents::codecs::pptx::{deck_model, lower_deck};
use systemprompt_files::documents::engine::loro::deck::{DeckShape, Frame, ShapeKind};
use systemprompt_files::documents::model::{Block, BlockKind, Mark, MarkKind};

use super::pptx_deck_lower::{base, shape_mut, texts};
use super::pptx_deck_model::{FRAMED, shape};
use super::pptx_fixture::part;

pub(super) const FRAME: Frame = Frame {
    x: 500,
    y: 600,
    w: 700,
    h: 800,
};

fn bullet() -> BlockKind {
    BlockKind::ListItem {
        level: 0,
        ordered: false,
        numbering: None,
        checked: None,
    }
}

pub(super) fn added(
    id: &str,
    kind: ShapeKind,
    frame: Option<Frame>,
    paragraphs: Vec<Block>,
) -> DeckShape {
    DeckShape {
        id: id.to_owned(),
        kind,
        frame,
        paragraphs,
        src: None,
    }
}

#[test]
fn moved_shapes_get_their_frames_written_whether_or_not_they_had_one() {
    let (bytes, mut deck) = base();
    for id in ["2", "3", "4", "shape-3", "6"] {
        shape_mut(&mut deck, FRAMED, id).frame = Some(FRAME);
    }
    let out = lower_deck(&bytes, &deck).expect("lower");
    let back = deck_model(&out).expect("deck");
    for id in ["2", "3", "4", "6"] {
        assert_eq!(shape(&back, FRAMED, id).frame, Some(FRAME), "shape {id}");
    }
    let xml = part(&out, FRAMED);
    assert_eq!(xml.matches("<a:off x=\"500\" y=\"600\"/>").count(), 5);
    assert_eq!(xml.matches("<a:ext cx=\"700\" cy=\"800\"/>").count(), 5);
}

#[test]
fn a_negative_extent_is_clamped_to_zero() {
    let (bytes, mut deck) = base();
    shape_mut(&mut deck, FRAMED, "2").frame = Some(Frame {
        w: -5,
        h: -1,
        ..FRAME
    });
    let out = lower_deck(&bytes, &deck).expect("lower");
    let frame = shape(&deck_model(&out).expect("deck"), FRAMED, "2").frame;
    assert_eq!(
        frame,
        Some(Frame {
            w: 0,
            h: 0,
            ..FRAME
        })
    );
}

#[test]
fn every_written_mark_round_trips_and_unwritten_ones_are_ignored() {
    let (bytes, mut deck) = base();
    let note = shape_mut(&mut deck, FRAMED, "4");
    let kinds = [
        MarkKind::Bold,
        MarkKind::Italic,
        MarkKind::Underline,
        MarkKind::Strike,
        MarkKind::Caps,
        MarkKind::Size {
            value: "18".to_owned(),
        },
        MarkKind::Color {
            value: "#00aa00".to_owned(),
        },
        MarkKind::Highlight {
            value: "ffff00".to_owned(),
        },
        MarkKind::Font {
            value: "A&B Sans".to_owned(),
        },
    ];
    note.paragraphs[0].marks = kinds
        .iter()
        .cloned()
        .map(|kind| Mark {
            start: 0,
            end: 2,
            kind,
        })
        .chain([
            Mark {
                start: 2,
                end: 3,
                kind: MarkKind::Superscript,
            },
            Mark {
                start: 3,
                end: 4,
                kind: MarkKind::Subscript,
            },
            Mark {
                start: 0,
                end: 4,
                kind: MarkKind::Link {
                    href: "https://x".to_owned(),
                },
            },
        ])
        .collect();
    let out = lower_deck(&bytes, &deck).expect("lower");
    let read = shape(&deck_model(&out).expect("deck"), FRAMED, "4").paragraphs[0].clone();
    assert_eq!(read.text, "Note");
    let at = |start: usize| -> Vec<&str> {
        let mut keys: Vec<&str> = read
            .marks
            .iter()
            .filter(|m| m.start == start)
            .map(|m| m.kind.key())
            .collect();
        keys.sort_unstable();
        keys
    };
    let mut wanted: Vec<&str> = kinds.iter().map(MarkKind::key).collect();
    wanted.sort_unstable();
    assert_eq!(at(0), wanted);
    assert_eq!(at(2), vec!["superscript"]);
    assert_eq!(at(3), vec!["subscript"]);
    assert!(read.marks.contains(&Mark {
        start: 0,
        end: 2,
        kind: MarkKind::Color {
            value: "#00AA00".to_owned()
        }
    }));
    assert!(part(&out, FRAMED).contains("typeface=\"A&amp;B Sans\""));
}

#[test]
fn removing_every_mark_keeps_the_base_run_properties() {
    let (bytes, mut deck) = base();
    shape_mut(&mut deck, FRAMED, "3").paragraphs[0]
        .marks
        .clear();
    let out = lower_deck(&bytes, &deck).expect("lower");
    let body = shape(&deck_model(&out).expect("deck"), FRAMED, "3").clone();
    assert!(body.paragraphs[0].marks.is_empty());
    assert_eq!(body.paragraphs[1].marks.len(), 3);
    assert!(part(&out, FRAMED).contains("<a:r><a:rPr lang=\"en-US\"/><a:t>Bold plain</a:t></a:r>"));
}

#[test]
fn paragraphs_become_and_stop_being_list_items() {
    let (bytes, mut deck) = base();
    shape_mut(&mut deck, FRAMED, "4").paragraphs[0].kind = bullet();
    shape_mut(&mut deck, FRAMED, "shape-3").paragraphs[0].kind = bullet();
    shape_mut(&mut deck, FRAMED, "3").paragraphs[0].kind = BlockKind::Paragraph;
    shape_mut(&mut deck, FRAMED, "3").paragraphs[2].kind = BlockKind::Paragraph;
    let out = lower_deck(&bytes, &deck).expect("lower");
    let back = deck_model(&out).expect("deck");
    assert!(matches!(
        shape(&back, FRAMED, "4").paragraphs[0].kind,
        BlockKind::ListItem { .. }
    ));
    assert!(matches!(
        shape(&back, FRAMED, "shape-3").paragraphs[0].kind,
        BlockKind::ListItem { .. }
    ));
    assert_eq!(
        shape(&back, FRAMED, "3").paragraphs[0].kind,
        BlockKind::Paragraph
    );
    assert_eq!(shape(&back, FRAMED, "3").paragraphs[2].text, "Hiring\nplan");
    assert_eq!(
        shape(&back, FRAMED, "3").paragraphs[2].kind,
        BlockKind::Paragraph
    );
    assert_eq!(lower_deck(&out, &back).expect("relower"), out);
    let xml = part(&out, FRAMED);
    assert!(xml.contains("<a:pPr lvl=\"1\"><a:buNone/></a:pPr>"));
    assert!(!xml.contains("<a:buNone/></a:pPr><a:r><a:t>Twin"));
}

#[test]
fn added_text_shapes_take_fresh_ids_frames_bullets_and_marks() {
    let (bytes, mut deck) = base();
    let mut styled = Block::new("n1", bullet(), "Point\nnext");
    styled.marks.push(Mark {
        start: 0,
        end: 5,
        kind: MarkKind::Bold,
    });
    let slide = deck
        .slides
        .iter_mut()
        .find(|s| s.id == FRAMED)
        .expect("slide");
    slide
        .shapes
        .push(added("a", ShapeKind::Text, Some(FRAME), vec![styled]));
    slide
        .shapes
        .push(added("b", ShapeKind::Title, None, Vec::new()));
    slide.shapes.push(added(
        "c",
        ShapeKind::Body,
        None,
        vec![Block::new("n2", BlockKind::Paragraph, "x")],
    ));
    slide
        .shapes
        .push(added("d", ShapeKind::Table, Some(FRAME), Vec::new()));
    let out = lower_deck(&bytes, &deck).expect("lower");
    let back = deck_model(&out).expect("deck");
    let read = back.slides.iter().find(|s| s.id == FRAMED).expect("slide");
    assert_eq!(read.shapes.len(), 8);
    let text = shape(&back, FRAMED, "7");
    assert_eq!(text.kind, ShapeKind::Text);
    assert_eq!(text.frame, Some(FRAME));
    assert_eq!(texts(text), vec!["Point\nnext"]);
    assert!(matches!(
        text.paragraphs[0].kind,
        BlockKind::ListItem { .. }
    ));
    assert_eq!(
        text.paragraphs[0].marks,
        vec![Mark {
            start: 0,
            end: 5,
            kind: MarkKind::Bold
        }]
    );
    assert_eq!(shape(&back, FRAMED, "8").kind, ShapeKind::Title);
    assert_eq!(shape(&back, FRAMED, "9").kind, ShapeKind::Body);
    assert!(part(&out, FRAMED).contains("txBox=\"1\""));
}
