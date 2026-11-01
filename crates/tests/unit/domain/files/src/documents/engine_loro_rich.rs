use std::error::Error;

use loro::{Container, LoroDoc, LoroMap, LoroText, LoroValue, ValueOrContainer};
use systemprompt_files::documents::engine::loro::rich::{self, ReadError};
use systemprompt_files::documents::engine::loro::seed_rich;
use systemprompt_files::documents::model::{
    Block, BlockKind, DocumentSettings, MarkKind, RichText,
};

fn doc_of(blocks: usize) -> LoroDoc {
    let model = RichText::of(
        (0..blocks)
            .map(|n| Block::new(format!("b{n}"), BlockKind::Paragraph, format!("text {n}")))
            .collect(),
    );
    seed_rich(&model).expect("seed")
}

fn first_block(doc: &LoroDoc) -> LoroMap {
    match rich::blocks(doc).get(0).expect("first block") {
        ValueOrContainer::Container(Container::Map(map)) => map,
        _ => panic!("block is a map"),
    }
}

fn settings_value(doc: &LoroDoc) -> Option<String> {
    match doc.get_map(rich::DOCUMENT_CONTAINER).get("settings")? {
        ValueOrContainer::Value(LoroValue::String(value)) => Some(value.to_string()),
        _ => None,
    }
}

fn front_matter(text: &str) -> DocumentSettings {
    DocumentSettings {
        front_matter: Some(text.to_owned()),
        ..DocumentSettings::default()
    }
}

#[test]
fn a_plain_value_in_the_block_list_fails_the_read() {
    let doc = doc_of(1);
    rich::blocks(&doc).push("not a map").expect("push");
    doc.commit();
    let error = rich::read(&doc).expect_err("not a block");
    assert!(matches!(error, ReadError::NotABlock { at: 1 }));
    assert_eq!(error.to_string(), "list entry 1 is not a block");
    assert!(error.source().is_none());
}

#[test]
fn a_block_without_a_kind_names_the_block() {
    let doc = doc_of(2);
    first_block(&doc).delete("kind").expect("drop kind");
    doc.commit();
    let error = rich::read(&doc).expect_err("no kind");
    assert!(matches!(&error, ReadError::MissingKind { id } if id == "b0"));
    assert_eq!(error.to_string(), "block b0 has no kind");
}

#[test]
fn a_missing_id_and_an_unreadable_kind_render_their_messages() {
    let doc = doc_of(1);
    first_block(&doc).delete("id").expect("drop id");
    doc.commit();
    assert_eq!(
        rich::read(&doc).expect_err("no id").to_string(),
        "block 0 has no id"
    );

    let doc = doc_of(1);
    first_block(&doc).insert("kind", "[1,2]").expect("kind");
    doc.commit();
    let error = rich::read(&doc).expect_err("bad kind");
    assert_eq!(error.to_string(), "block b0 has an unreadable kind");
    assert!(error.source().is_some());
}

#[test]
fn settings_are_written_once_and_removed_when_back_to_default() {
    let doc = doc_of(1);
    rich::sync_settings(&doc, Some(&front_matter("title: a"))).expect("write");
    doc.commit();
    let held = settings_value(&doc).expect("held");
    assert!(held.contains("title: a"), "{held}");
    let version = doc.oplog_vv();
    rich::sync_settings(&doc, Some(&front_matter("title: a"))).expect("unchanged");
    doc.commit();
    assert_eq!(doc.oplog_vv(), version);
    assert_eq!(
        rich::read(&doc).expect("read").document.as_deref(),
        Some(&front_matter("title: a"))
    );

    rich::sync_settings(&doc, Some(&DocumentSettings::default())).expect("clear");
    doc.commit();
    assert_eq!(settings_value(&doc), None);
    rich::sync_settings(&doc, None).expect("still clear");
    assert_eq!(settings_value(&doc), None);
}

#[test]
fn unreadable_settings_fall_back_to_the_base_settings() {
    let doc = doc_of(1);
    doc.get_map(rich::DOCUMENT_CONTAINER)
        .insert("settings", "{not json")
        .expect("insert");
    doc.commit();
    let model = rich::read(&doc).expect("read");
    assert_eq!(model.document, None);
    assert_eq!(model.blocks.len(), 1);
}

#[test]
fn read_text_merges_adjacent_runs_of_one_mark() {
    let doc = LoroDoc::new();
    rich::configure(&doc);
    let text: LoroText = doc.get_text("t");
    text.insert(0, "abcdef").expect("insert");
    text.mark(0..2, "bold", true).expect("bold");
    text.mark(2..4, "bold", true).expect("bold again");
    text.mark(4..6, "link", "https://example.com")
        .expect("link");
    doc.commit();
    let (plain, marks) = rich::read_text(&text);
    assert_eq!(plain, "abcdef");
    let bold: Vec<(usize, usize)> = marks
        .iter()
        .filter(|m| m.kind == MarkKind::Bold)
        .map(|m| (m.start, m.end))
        .collect();
    assert_eq!(bold, vec![(0, 4)]);
    assert!(marks.iter().any(|m| m.kind
        == MarkKind::Link {
            href: "https://example.com".to_owned()
        }
        && (m.start, m.end) == (4, 6)));
    assert_eq!(marks.len(), 2);
}
