use loro::{Container, LoroMap, ValueOrContainer};
use systemprompt_files::documents::engine::loro::LoroEngine;
use systemprompt_files::documents::engine::loro::deck::{
    self, DeckInvalid, DeckModel, DeckReadError, DeckShape, DeckSlide, Frame, ShapeKind, seed_deck,
    validate_deck_changes, write_deck,
};
use systemprompt_files::documents::engine::{DocumentEngine, ModelView};
use systemprompt_files::documents::model::{Block, BlockKind};

fn shape(id: &str, kind: ShapeKind, text: &str) -> DeckShape {
    DeckShape {
        id: id.to_owned(),
        kind,
        frame: Some(Frame {
            x: 10,
            y: 20,
            w: 300,
            h: 40,
        }),
        paragraphs: vec![Block::new(format!("{id}.p"), BlockKind::Paragraph, text)],
        src: None,
    }
}

fn slide(id: &str, title: &str) -> DeckSlide {
    DeckSlide {
        id: id.to_owned(),
        layout: Some("title".to_owned()),
        shapes: vec![
            shape(&format!("{id}.t"), ShapeKind::Title, title),
            DeckShape {
                id: format!("{id}.img"),
                kind: ShapeKind::Image,
                frame: None,
                paragraphs: Vec::new(),
                src: Some("blob-1".to_owned()),
            },
        ],
    }
}

fn deck_of(slides: Vec<DeckSlide>) -> DeckModel {
    DeckModel { slides }
}

fn map_at(list: &loro::LoroMovableList, at: usize) -> LoroMap {
    match list.get(at).expect("entry") {
        ValueOrContainer::Container(Container::Map(map)) => map,
        _ => panic!("map"),
    }
}

fn first_shape(doc: &loro::LoroDoc) -> LoroMap {
    let slide = map_at(&deck::slides(doc), 0);
    let ValueOrContainer::Container(Container::MovableList(shapes)) =
        slide.get("shapes").expect("shapes")
    else {
        panic!("shapes list");
    };
    map_at(&shapes, 0)
}

#[test]
fn engine_loro_shape_kinds_name_round_trip() {
    for kind in [
        ShapeKind::Title,
        ShapeKind::Body,
        ShapeKind::Text,
        ShapeKind::Table,
        ShapeKind::Image,
        ShapeKind::Other,
    ] {
        assert_eq!(ShapeKind::from_name(kind.name()), Some(kind));
    }
    assert_eq!(ShapeKind::from_name("chart"), None);
    assert!(ShapeKind::Body.holds_text());
    assert!(!ShapeKind::Image.holds_text());
}

#[test]
fn engine_loro_a_deck_round_trips_through_the_engine() {
    let model = ModelView::Deck(deck_of(vec![slide("s1", "One"), slide("s2", "Two")]));
    let doc = LoroEngine.seed(&model).expect("seed");
    assert_eq!(doc.export_model().expect("export"), model);
    let reopened = LoroEngine
        .open(&doc.snapshot().expect("snapshot"))
        .expect("open");
    assert_eq!(reopened.export_model().expect("export"), model);
}

#[test]
fn engine_loro_write_deck_moves_edits_and_removes_by_id() {
    let doc = seed_deck(&deck_of(vec![
        slide("s1", "One"),
        slide("s2", "Two"),
        slide("s3", "Three"),
    ]))
    .expect("seed");
    let mut edited = slide("s1", "One, edited");
    edited.layout = None;
    edited.shapes[0].kind = ShapeKind::Text;
    edited.shapes[0].frame = None;
    edited.shapes[1].src = None;
    edited.shapes[1].frame = Some(Frame {
        x: 0,
        y: 0,
        w: 5,
        h: 5,
    });
    edited.shapes.reverse();
    let target = deck_of(vec![slide("s3", "Three"), edited, slide("s4", "Four")]);
    write_deck(&doc, &target).expect("write");
    doc.commit();
    assert_eq!(deck::read(&doc).expect("read"), target);
}

#[test]
fn engine_loro_concurrent_deck_edits_converge() {
    let seed = ModelView::Deck(deck_of(vec![slide("s1", "One"), slide("s2", "Two")]));
    let mut left = LoroEngine.seed(&seed).expect("seed");
    let mut right = LoroEngine
        .open(&left.snapshot().expect("snapshot"))
        .expect("open");
    let from_left = left
        .update_to_model(&ModelView::Deck(deck_of(vec![
            slide("s1", "One!"),
            slide("s2", "Two"),
        ])))
        .expect("diff")
        .expect("changed");
    let from_right = right
        .update_to_model(&ModelView::Deck(deck_of(vec![
            slide("s1", "One"),
            slide("s2", "Two"),
            slide("s3", "Three"),
        ])))
        .expect("diff")
        .expect("changed");
    for doc in [&mut left, &mut right] {
        doc.apply_update(&from_left).expect("left");
        doc.apply_update(&from_right).expect("right");
    }
    let expected = ModelView::Deck(deck_of(vec![
        slide("s1", "One!"),
        slide("s2", "Two"),
        slide("s3", "Three"),
    ]));
    assert_eq!(left.export_model().expect("export"), expected);
    assert_eq!(right.export_model().expect("export"), expected);
}

#[test]
fn engine_loro_unreadable_deck_entries_name_their_path() {
    let doc = seed_deck(&deck_of(vec![slide("s1", "One")])).expect("seed");
    first_shape(&doc).insert("kind", "chart").expect("kind");
    assert!(matches!(
        deck::read(&doc),
        Err(DeckReadError::Kind { path }) if path == "slide 0 shape 0"
    ));
    first_shape(&doc).insert("kind", "title").expect("kind");
    let ValueOrContainer::Container(Container::Map(frame)) =
        first_shape(&doc).get("frame").expect("frame")
    else {
        panic!("frame map");
    };
    frame.insert("w", "wide").expect("w");
    assert!(matches!(deck::read(&doc), Err(DeckReadError::Frame { .. })));
    frame.insert("w", 1.6_f64).expect("w");
    assert_eq!(
        deck::read(&doc).expect("read").slides[0].shapes[0]
            .frame
            .map(|f| f.w),
        Some(2)
    );
    map_at(&deck::slides(&doc), 0)
        .delete("id")
        .expect("drop id");
    assert!(matches!(
        validate_deck_changes(&doc),
        Err(DeckInvalid::Read(DeckReadError::MissingId { path })) if path == "slide 0"
    ));
    deck::slides(&doc).insert(0, "not a map").expect("value");
    assert!(matches!(
        deck::read(&doc),
        Err(DeckReadError::NotAMap { .. })
    ));
}

#[test]
fn engine_loro_deck_validation_refuses_duplicates_and_negative_frames() {
    let good = seed_deck(&deck_of(vec![slide("s1", "One")])).expect("seed");
    assert_eq!(
        validate_deck_changes(&good).expect("valid"),
        deck_of(vec![slide("s1", "One")])
    );
    let twice = seed_deck(&deck_of(vec![slide("s1", "One"), slide("s1", "Again")])).expect("seed");
    assert!(matches!(
        validate_deck_changes(&twice),
        Err(DeckInvalid::DuplicateId { id }) if id == "s1"
    ));
    let mut negative = slide("s1", "One");
    negative.shapes[0].frame = Some(Frame {
        x: 0,
        y: 0,
        w: -1,
        h: 1,
    });
    let doc = seed_deck(&deck_of(vec![negative])).expect("seed");
    assert!(matches!(
        validate_deck_changes(&doc),
        Err(DeckInvalid::NegativeFrame { shape }) if shape == "s1.t"
    ));
}
