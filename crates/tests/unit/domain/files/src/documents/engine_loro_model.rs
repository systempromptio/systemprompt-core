use loro::{Container, ValueOrContainer};
use systemprompt_engine_loro::rich::{self, ReadError};
use systemprompt_engine_loro::{seed_rich, write_model};
use systemprompt_files::documents::model::{Block, BlockKind, RichText};

fn paragraphs(ids: impl IntoIterator<Item = usize>) -> RichText {
    RichText::of(
        ids.into_iter()
            .map(|n| Block::new(format!("b{n}"), BlockKind::Paragraph, format!("text {n}")))
            .collect(),
    )
}

fn first_block(doc: &loro::LoroDoc) -> loro::LoroMap {
    match rich::blocks(doc).get(0).expect("first block") {
        ValueOrContainer::Container(Container::Map(map)) => map,
        _ => panic!("block is a map"),
    }
}

#[test]
fn an_unreadable_block_kind_fails_the_read() {
    let doc = seed_rich(&paragraphs(0..3)).expect("seed");
    first_block(&doc)
        .insert("kind", "{\"not\":\"a kind\"}")
        .expect("corrupt kind");
    doc.commit();
    match rich::read(&doc) {
        Err(ReadError::Kind { id, .. }) => assert_eq!(id, "b0"),
        other => panic!("expected a kind error, got {other:?}"),
    }
}

#[test]
fn a_block_without_an_id_fails_the_read() {
    let doc = seed_rich(&paragraphs(0..2)).expect("seed");
    first_block(&doc).delete("id").expect("drop id");
    doc.commit();
    assert!(matches!(
        rich::read(&doc),
        Err(ReadError::MissingId { at: 0 })
    ));
}

#[test]
fn a_large_document_is_rewritten_into_any_order() {
    let doc = seed_rich(&paragraphs(0..5000)).expect("seed");
    let mut target: Vec<usize> = (0..5000).filter(|n| n % 7 != 0).collect();
    target.reverse();
    target.extend(5000..5100);
    target.swap(10, 4000);
    let mut model = paragraphs(target.iter().copied());
    model.blocks[20].text = "edited".to_owned();
    write_model(&doc, &model).expect("write");
    doc.commit();
    assert_eq!(rich::read(&doc).expect("read"), model);
}

#[test]
fn repeated_ids_in_the_document_or_the_target_stay_in_step() {
    let doc = seed_rich(&paragraphs([1, 2, 1, 3])).expect("seed");
    let model = paragraphs([3, 1, 2, 2]);
    write_model(&doc, &model).expect("write");
    doc.commit();
    assert_eq!(rich::read(&doc).expect("read"), model);
}
