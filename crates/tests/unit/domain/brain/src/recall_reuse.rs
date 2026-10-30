use chrono::{TimeZone, Utc};
use systemprompt_brain::recall::reuse::parse_stored;
use systemprompt_extension::brain::{InjectedFact, RecallBlock};

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

#[test]
fn a_stored_block_parses_back_to_the_identical_bytes() {
    let rendered = block().render();
    let parsed = parse_stored(&rendered).expect("parses");
    assert_eq!(parsed.render(), rendered);
    assert_eq!(parsed.facts.len(), 2);
    assert_eq!(parsed.scope, "acme & co");
}

#[test]
fn text_that_is_not_a_block_is_not_reused() {
    assert!(parse_stored("hello").is_none());
    let mut rendered = block().render();
    rendered.push(' ');
    assert!(parse_stored(&rendered).is_none());
}
