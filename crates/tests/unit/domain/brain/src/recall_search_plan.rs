use std::collections::HashMap;

use systemprompt_brain::recall::search_plan::{SearchPolicy, plan};
use systemprompt_brain::repository::SearchTerms;
use systemprompt_models::brain::SynonymsDoc;

fn synonyms() -> SynonymsDoc {
    SynonymsDoc {
        groups: vec![
            vec!["catalogue".to_owned(), "catalog".to_owned()],
            vec!["sfcc".to_owned(), "commerce cloud".to_owned()],
        ],
    }
}

fn terms(pairs: &[(&str, u64)], total: u64) -> SearchTerms {
    SearchTerms {
        total,
        ndoc: pairs
            .iter()
            .map(|(w, n)| ((*w).to_owned(), *n))
            .collect::<HashMap<_, _>>(),
    }
}

fn policy() -> SearchPolicy {
    SearchPolicy {
        max_df: 0.2,
        quorum: 0.5,
    }
}

#[test]
fn common_words_only_rank_and_the_rarest_half_is_required() {
    let terms = terms(
        &[
            ("cloud", 900),
            ("sfcc", 600),
            ("import", 90),
            ("catalogue", 200),
            ("calloway", 18),
        ],
        1000,
    );
    let plan = plan(
        "Any issues with catalogue import on calloway sfcc",
        &synonyms(),
        &terms,
        &policy(),
    )
    .unwrap();
    // informative: calloway (0.018), import (0.09), catalogue group (0.2);
    // "issues" and "with" occur in no fact and "sfcc" is too common.
    assert_eq!(plan.filter, "calloway & import");
    assert_eq!(
        plan.rank,
        "any | issues | with | (catalogue | catalog) | import | calloway | (sfcc | commerce <-> cloud)"
    );
}

#[test]
fn at_least_one_group_is_required_and_a_lone_common_message_only_ranks() {
    let terms = terms(&[("renewal", 50), ("cloud", 900)], 1000);
    let only_rare = plan("renewal", &synonyms(), &terms, &policy()).unwrap();
    assert_eq!(only_rare.filter, "renewal");
    let only_common = plan("cloud cloud", &synonyms(), &terms, &policy()).unwrap();
    assert_eq!(only_common.filter, "cloud");
    assert_eq!(only_common.rank, "cloud");
}

#[test]
fn without_statistics_every_group_is_optional() {
    let plan = plan(
        "catalogue import",
        &synonyms(),
        &SearchTerms::default(),
        &policy(),
    )
    .unwrap();
    assert_eq!(plan.filter, "(catalogue | catalog) | import");
    assert_eq!(plan.filter, plan.rank);
}

#[test]
fn short_and_absent_messages_plan_nothing() {
    assert!(plan("a of", &synonyms(), &SearchTerms::default(), &policy()).is_none());
    assert!(plan("", &synonyms(), &SearchTerms::default(), &policy()).is_none());
}
