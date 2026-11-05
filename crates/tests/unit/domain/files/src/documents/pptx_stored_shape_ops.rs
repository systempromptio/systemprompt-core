use systemprompt_files::documents::codecs::pptx::deck_model;
use systemprompt_files::documents::engine::loro::deck::{DeckModel, Frame};
use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{CodecError, DocumentCodec, EditOp};

use super::pptx_deck_model::FRAMED;
use super::pptx_fixture::{framed_deck, project};
use systemprompt_files::documents::codecs::pptx::PptxCodec;

fn framed_shape(deck: &DeckModel) -> (usize, usize, Frame) {
    let slide = deck.slides.iter().position(|s| s.id == FRAMED).expect("slide");
    let (shape, frame) = deck.slides[slide]
        .shapes
        .iter()
        .enumerate()
        .find_map(|(at, shape)| shape.frame.map(|frame| (at, frame)))
        .expect("a framed shape");
    (slide, shape, frame)
}

fn node(slide: usize, shape: usize) -> DocumentNodeId {
    DocumentNodeId::new(format!("slide[{}]/shape[{shape}]", slide + 1))
}

fn run(bytes: &[u8], op: EditOp) -> Result<Vec<u8>, CodecError> {
    PptxCodec.apply(bytes, &project(bytes), &[op])
}

#[test]
fn a_stored_deck_moves_and_resizes_a_shape_with_no_open_session() {
    let bytes = framed_deck();
    let (slide, shape, start) = framed_shape(&deck_model(&bytes).expect("deck"));
    let moved = run(
        &bytes,
        EditOp::MoveShape {
            node: node(slide, shape),
            x: start.x + 1000,
            y: start.y + 2000,
        },
    )
    .expect("move");
    let (_, _, frame) = framed_shape(&deck_model(&moved).expect("deck"));
    assert_eq!((frame.x, frame.y), (start.x + 1000, start.y + 2000));
    assert_eq!((frame.w, frame.h), (start.w, start.h));

    let resized = run(
        &moved,
        EditOp::ResizeShape {
            node: node(slide, shape),
            w: start.w + 500,
            h: start.h + 700,
        },
    )
    .expect("resize");
    let (_, _, frame) = framed_shape(&deck_model(&resized).expect("deck"));
    assert_eq!((frame.w, frame.h), (start.w + 500, start.h + 700));
    assert_eq!((frame.x, frame.y), (start.x + 1000, start.y + 2000));
}

#[test]
fn a_stored_deck_deletes_a_shape_and_refuses_bad_ops() {
    let bytes = framed_deck();
    let deck = deck_model(&bytes).expect("deck");
    let (slide, shape, _) = framed_shape(&deck);
    let held = deck.slides[slide].shapes.len();
    let deleted = run(
        &bytes,
        EditOp::DeleteShape {
            node: node(slide, shape),
        },
    )
    .expect("delete");
    let after = deck_model(&deleted).expect("deck");
    assert_eq!(after.slides[slide].shapes.len(), held - 1);

    assert!(matches!(
        run(
            &bytes,
            EditOp::ResizeShape {
                node: node(slide, shape),
                w: 0,
                h: 10,
            },
        ),
        Err(CodecError::Refused { .. })
    ));
    assert!(matches!(
        run(
            &bytes,
            EditOp::MoveShape {
                node: DocumentNodeId::new("slide[99]/shape[0]"),
                x: 0,
                y: 0,
            },
        ),
        Err(CodecError::UnknownNode { .. })
    ));
}
