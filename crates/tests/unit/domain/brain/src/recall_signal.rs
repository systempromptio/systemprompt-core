use systemprompt_brain::recall::signal::{asked_kinds, names, subject_matched, withhold_full_text};
use systemprompt_extension::brain::TierIds;
use systemprompt_models::brain::{KindDecl, RetrievalTier};

use crate::fixtures::kind;
use crate::recall_fixtures::id;

fn decl(id: &str, asks: &[&str]) -> KindDecl {
    KindDecl {
        kind: kind(id),
        subject: "project".to_owned(),
        description: None,
        fields: std::collections::BTreeMap::new(),
        topics: Vec::new(),
        statement: "{subject}".to_owned(),
        identity: vec!["subject".to_owned()],
        materiality: systemprompt_models::brain::Materiality::default(),
        review_after_days: None,
        extractor: None,
        asks: asks.iter().map(|w| (*w).to_owned()).collect(),
    }
}

fn kinds() -> Vec<KindDecl> {
    vec![
        decl("decision", &["decisions", "decided"]),
        decl("delivery_risk", &["risks"]),
        decl("renewal", &[]),
    ]
}

#[test]
fn a_kind_is_asked_by_its_declared_asks_or_else_its_id_words() {
    assert_eq!(
        asked_kinds("Remind me of the DECISIONS taken here", &kinds()),
        vec![kind("decision")]
    );
    assert!(
        asked_kinds("what delivery looks like", &kinds()).is_empty(),
        "a declared list replaces the id words"
    );
    assert_eq!(
        asked_kinds("what delivery looks like", &[decl("delivery_risk", &[])]),
        vec![kind("delivery_risk")],
        "one word of the id is enough when nothing is declared"
    );
    assert_eq!(
        asked_kinds("risks and renewal", &kinds()),
        vec![kind("delivery_risk"), kind("renewal")]
    );
    assert!(asked_kinds("renewals", &kinds()).is_empty(), "no stemming");
    assert!(asked_kinds("", &kinds()).is_empty());
}

#[test]
fn a_name_is_capitalised_not_sentence_initial_and_three_letters_or_more() {
    let stop = vec!["the".to_owned()];
    assert_eq!(
        names(
            "Catch me up on Zorblax. Also Hollowell MSA? The PIM too",
            &stop
        ),
        vec!["zorblax", "hollowell", "msa", "pim"]
    );
    assert!(names("Zorblax", &stop).is_empty(), "sentence-initial");
    assert!(
        names("what about Z2 or Ab", &stop).is_empty(),
        "digits, short"
    );
    assert!(names("on The road", &stop).is_empty(), "a stop word");
    assert_eq!(names("on Acme and Acme", &stop), vec!["acme"], "once");
    assert_eq!(
        names("risks on Ashgrove's SFCC replatform", &stop),
        vec!["ashgrove", "sfcc"],
        "a possessive is the name it owns"
    );
}

#[test]
fn withholding_the_full_text_tier_counts_only_the_ids_no_other_tier_found() {
    let mut tiers = vec![
        TierIds::new(RetrievalTier::Context, vec![id(1)]),
        TierIds::new(RetrievalTier::FullText, vec![id(1), id(2), id(3)]),
    ];
    assert!(!subject_matched(&tiers));
    assert_eq!(withhold_full_text(&mut tiers), 2);
    assert!(tiers[1].ids.is_empty());
    assert_eq!(tiers[0].ids, vec![id(1)]);
    let named = vec![TierIds::new(RetrievalTier::Linked, vec![id(4)])];
    assert!(subject_matched(&named));
}
