use chrono::{TimeZone, Utc};
use systemprompt_brain::recall::reuse::parse_stored;
use systemprompt_extension::brain::{InjectedFact, RecallBlock};
use systemprompt_models::brain::{FactRef, StoredBlock};

use crate::recall_fixtures::id;

fn block() -> RecallBlock {
    RecallBlock::new(
        "acme & co",
        Utc.with_ymd_and_hms(2026, 10, 30, 9, 0, 0)
            .single()
            .expect("instant"),
    )
    .with_fact(InjectedFact {
        id: id(1),
        revision: 3,
        source: "doc:d1@r2".to_owned(),
        text: "Billing <owner> is Ana & Bo".to_owned(),
    })
    .with_fact(InjectedFact {
        id: id(2),
        revision: 1,
        source: "entered:u1".to_owned(),
        text: "Release freeze on Fridays".to_owned(),
    })
}

fn stored(block: &RecallBlock) -> StoredBlock {
    StoredBlock {
        rendered: block.render(),
        as_of: block.as_of,
        facts: block
            .facts
            .iter()
            .map(|fact| FactRef {
                id: fact.id.clone(),
                revision: fact.revision,
            })
            .collect(),
    }
}

#[test]
fn a_stored_block_parses_back_to_the_identical_bytes() {
    let stored = stored(&block());
    let parsed = parse_stored(&stored).expect("parses");
    assert_eq!(parsed.render(), stored.rendered);
    assert_eq!(parsed.facts.len(), 2);
    assert_eq!(parsed.scope, "acme & co");
}

#[test]
fn a_reused_block_names_each_fact_at_its_stored_revision() {
    let original = block();
    let parsed = parse_stored(&stored(&original)).expect("parses");
    assert_eq!(parsed.ids(), format!("{}@3,{}@1", id(1), id(2)));
    assert_eq!(parsed.ids(), original.ids());
}

#[test]
fn text_that_is_not_a_block_is_not_reused() {
    let mut not_a_block = stored(&block());
    not_a_block.rendered = "hello".to_owned();
    assert!(parse_stored(&not_a_block).is_none());
    let mut trailing = stored(&block());
    trailing.rendered.push(' ');
    assert!(parse_stored(&trailing).is_none());
}

#[test]
fn refs_that_disagree_with_the_text_are_not_reused() {
    let mut missing = stored(&block());
    missing.facts.pop();
    assert!(parse_stored(&missing).is_none());
    let mut extra = stored(&block());
    extra.facts.push(FactRef {
        id: id(3),
        revision: 1,
    });
    assert!(parse_stored(&extra).is_none());
    let mut swapped = stored(&block());
    swapped.facts.reverse();
    assert!(parse_stored(&swapped).is_none());
}
