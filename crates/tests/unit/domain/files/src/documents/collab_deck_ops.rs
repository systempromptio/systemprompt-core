use systemprompt_files::documents::engine::loro::LoroEngine;
use systemprompt_files::documents::engine::loro::deck::{
    DeckModel, DeckShape, DeckSlide, ShapeKind,
};
use systemprompt_files::documents::engine::{DocumentEngine, EngineDoc, ModelView};
use systemprompt_files::documents::model::{Block, BlockKind};
use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{ContentAs, EditOp, NodeKind, NodeTarget};

const PEER: u64 = 777;

fn shape(kind: ShapeKind, lines: &[&str]) -> DeckShape {
    DeckShape {
        id: format!("s-{}", kind.name()),
        kind,
        frame: None,
        paragraphs: lines
            .iter()
            .enumerate()
            .map(|(i, text)| Block::new(format!("p{i}"), BlockKind::Paragraph, *text))
            .collect(),
        src: None,
    }
}

fn seeded() -> Box<dyn EngineDoc> {
    let deck = DeckModel {
        slides: vec![DeckSlide {
            id: "slide-one".to_owned(),
            layout: None,
            shapes: vec![
                shape(ShapeKind::Title, &["Opening"]),
                shape(ShapeKind::Body, &["alpha", "beta"]),
            ],
        }],
    };
    LoroEngine.seed(&ModelView::Deck(deck)).expect("seed")
}

fn applied(ops: &[EditOp]) -> DeckModel {
    let doc = seeded();
    let mut replica = doc.fork();
    let update = doc
        .update_from_ops(ops, PEER)
        .expect("translate")
        .expect("changed");
    replica.apply_update(&update.update).expect("import");
    match replica.export_model().expect("export") {
        ModelView::Deck(deck) => deck,
        other => panic!("not a deck: {other:?}"),
    }
}

fn lines(deck: &DeckModel, slide: usize) -> Vec<String> {
    deck.slides[slide]
        .shapes
        .iter()
        .flat_map(|shape| shape.paragraphs.iter().map(|p| p.text.clone()))
        .collect()
}

fn node(id: &str) -> NodeTarget {
    NodeTarget::Id(DocumentNodeId::new(id))
}

#[test]
fn replace_text_rewrites_one_paragraph() {
    let deck = applied(&[EditOp::ReplaceText {
        node: node("slide[1]/shape[1]/p[0]"),
        expected_hash: None,
        text: "gamma".to_owned(),
        content_as: ContentAs::Text,
    }]);
    assert_eq!(lines(&deck, 0), ["Opening", "gamma", "beta"]);
}

#[test]
fn str_replace_rewrites_the_unique_quote() {
    let deck = applied(&[EditOp::StrReplace {
        old_str: "Open".to_owned(),
        new_str: "Clos".to_owned(),
    }]);
    assert_eq!(lines(&deck, 0), ["Closing", "alpha", "beta"]);
}

#[test]
fn insert_after_adds_one_paragraph_per_line() {
    let deck = applied(&[EditOp::InsertAfter {
        node: node("slide[1]/shape[1]/p[0]"),
        kind: NodeKind::ListItem,
        text: "one\ntwo".to_owned(),
        content_as: ContentAs::Text,
    }]);
    assert_eq!(lines(&deck, 0), ["Opening", "alpha", "one", "two", "beta"]);
}

#[test]
fn delete_removes_a_paragraph_or_a_slide() {
    let deck = applied(&[EditOp::Delete {
        node: node("slide[1]/shape[1]/p[1]"),
        expected_hash: None,
    }]);
    assert_eq!(lines(&deck, 0), ["Opening", "alpha"]);
    let deck = applied(&[
        EditOp::InsertSlide {
            after: None,
            layout: None,
            title: Some("Second".to_owned()),
            body: Vec::new(),
        },
        EditOp::Delete {
            node: node("slide[1]"),
            expected_hash: None,
        },
    ]);
    assert_eq!(deck.slides.len(), 1);
    assert_eq!(lines(&deck, 0), ["Second"]);
}

#[test]
fn insert_slide_places_title_and_body_after_the_named_slide() {
    let deck = applied(&[
        EditOp::InsertSlide {
            after: None,
            layout: Some("blank".to_owned()),
            title: Some("Last".to_owned()),
            body: Vec::new(),
        },
        EditOp::InsertSlide {
            after: Some(DocumentNodeId::new("slide[1]")),
            layout: None,
            title: Some("Middle".to_owned()),
            body: vec!["point a".to_owned(), "point b".to_owned()],
        },
    ]);
    assert_eq!(deck.slides.len(), 3);
    assert_eq!(lines(&deck, 1), ["Middle", "point a", "point b"]);
    assert_eq!(lines(&deck, 2), ["Last"]);
    assert_eq!(deck.slides[2].layout.as_deref(), Some("blank"));
}

#[test]
fn unknown_targets_and_ambiguous_quotes_are_refused() {
    let doc = seeded();
    let before = doc.version();
    let refused = [
        EditOp::Delete {
            node: node("slide[5]"),
            expected_hash: None,
        },
        EditOp::Delete {
            node: node("cover"),
            expected_hash: None,
        },
        EditOp::ReplaceText {
            node: node("slide[1]/shape[9]/p[1]"),
            expected_hash: None,
            text: "x".to_owned(),
            content_as: ContentAs::Text,
        },
        EditOp::InsertSlide {
            after: Some(DocumentNodeId::new("slide[3]")),
            layout: None,
            title: None,
            body: Vec::new(),
        },
        EditOp::StrReplace {
            old_str: "a".to_owned(),
            new_str: "x".to_owned(),
        },
        EditOp::StrReplace {
            old_str: String::new(),
            new_str: "x".to_owned(),
        },
        EditOp::SetText {
            text: "whole".to_owned(),
        },
    ];
    for op in refused {
        assert!(
            doc.update_from_ops(std::slice::from_ref(&op), PEER)
                .is_err(),
            "{op:?}"
        );
        assert_eq!(doc.version(), before);
    }
}
