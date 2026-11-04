use std::collections::HashMap;

use systemprompt_brain::recall::search_plan::{SearchPolicy, plan};
use systemprompt_brain::repository::SearchTerms;
use systemprompt_models::brain::{QuorumUnit, SynonymsDoc};

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
    SearchPolicy::new(0.2, 0.5)
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

#[test]
fn a_retry_policy_exists_only_when_it_would_require_fewer_groups() {
    let mut policy = policy();
    assert_eq!(policy.retry(), None, "no retry quorum declared");
    policy.retry_quorum = 0.34;
    let retry = policy.retry().expect("a second pass");
    assert!((retry.quorum - 0.34).abs() < f64::EPSILON);
    assert_eq!(retry.retry(), None, "the second pass has no third");
    policy.retry_quorum = 0.5;
    assert_eq!(
        policy.retry(),
        None,
        "an equal quorum plans the same filter"
    );
}

#[test]
fn the_second_pass_requires_fewer_groups_than_the_first() {
    let terms = terms(
        &[
            ("adopt", 40),
            ("akeneo", 12),
            ("pim", 30),
            ("product", 150),
            ("master", 90),
        ],
        1000,
    );
    let first = plan(
        "adopt Akeneo PIM as the product master",
        &synonyms(),
        &terms,
        &policy(),
    )
    .unwrap();
    assert_eq!(first.filter, "akeneo & pim & adopt");
    let retry = SearchPolicy {
        retry_quorum: 0.34,
        ..policy()
    }
    .retry()
    .unwrap();
    let second = plan(
        "adopt Akeneo PIM as the product master",
        &synonyms(),
        &terms,
        &retry,
    )
    .unwrap();
    assert_eq!(second.filter, "akeneo & pim");
    assert_eq!(second.rank, first.rank);
}

#[test]
fn a_word_quorum_counts_every_query_word_a_group_absorbed() {
    let terms = terms(
        &[("catalogue", 100), ("catalog", 20), ("renewal", 50)],
        1000,
    );
    let message = "catalogue catalog renewal";
    let groups = plan(message, &synonyms(), &terms, &policy()).unwrap();
    assert_eq!(
        groups.filter, "renewal",
        "two groups, half rounded up is one"
    );
    let words = plan(
        message,
        &synonyms(),
        &terms,
        &SearchPolicy {
            quorum_over: QuorumUnit::Words,
            ..policy()
        },
    )
    .unwrap();
    assert_eq!(
        words.filter, "renewal & (catalogue | catalog)",
        "three words, half rounded up is two groups"
    );
}

fn stop_words() -> Vec<String> {
    ["call", "what", "the", "on"]
        .into_iter()
        .map(str::to_owned)
        .collect()
}

#[test]
fn a_stop_word_is_never_required_and_never_ranks() {
    let terms = terms(
        &[("call", 17), ("akeneo", 12), ("pim", 30), ("the", 800)],
        1000,
    );
    let message = "What's the call on the Akeneo PIM";
    let scaffolded = plan(message, &synonyms(), &terms, &policy()).unwrap();
    assert_eq!(scaffolded.filter, "akeneo & call");
    let planned = plan(
        message,
        &synonyms(),
        &terms,
        &policy().with_stop_words(stop_words()),
    )
    .unwrap();
    assert_eq!(
        planned.filter, "akeneo",
        "two groups, half rounded up is one"
    );
    assert_eq!(planned.rank, "akeneo | pim");
    let all = plan(
        message,
        &synonyms(),
        &terms,
        &SearchPolicy::new(0.2, 1.0).with_stop_words(stop_words()),
    )
    .unwrap();
    assert_eq!(all.filter, "akeneo & pim");
}

#[test]
fn a_message_of_stop_words_and_common_words_plans_nothing() {
    let terms = terms(&[("the", 800), ("delivery", 300)], 1000);
    let stopped = policy().with_stop_words(stop_words());
    assert_eq!(plan("What's the call", &synonyms(), &terms, &stopped), None);
    assert_eq!(
        plan("the delivery", &synonyms(), &terms, &stopped),
        None,
        "a common word alone narrows nothing"
    );
    let undeclared = plan("the delivery", &synonyms(), &terms, &policy()).unwrap();
    assert_eq!(undeclared.filter, "the | delivery");
}

#[test]
fn without_statistics_stop_words_leave_the_disjunction() {
    let planned = plan(
        "the catalogue import",
        &synonyms(),
        &SearchTerms::default(),
        &policy().with_stop_words(stop_words()),
    )
    .unwrap();
    assert_eq!(planned.filter, "(catalogue | catalog) | import");
}

#[test]
fn a_filter_whose_rarest_required_word_is_too_common_plans_nothing() {
    let terms = terms(&[("client", 30_000), ("renewal", 50)], 300_000);
    let capped = policy().with_max_ndoc(5_000);
    let narrowed = plan("client renewal", &synonyms(), &terms, &capped).unwrap();
    assert_eq!(
        narrowed.filter, "renewal",
        "the rarest word is within the cap"
    );
    assert_eq!(
        plan("client", &synonyms(), &terms, &capped),
        None,
        "thirty thousand rows are not worth ranking"
    );
    let open = plan("client", &synonyms(), &terms, &policy()).unwrap();
    assert_eq!(open.filter, "client", "0 keeps the plan");
    assert_eq!(
        plan(
            "client",
            &synonyms(),
            &terms,
            &policy().with_max_ndoc(30_000)
        )
        .unwrap()
        .filter,
        "client",
        "the cap is inclusive"
    );
}

#[test]
fn a_disjunction_is_bounded_by_the_sum_of_its_groups() {
    let terms = terms(&[("client", 300), ("product", 250)], 1000);
    let both = plan("client product", &synonyms(), &terms, &policy()).unwrap();
    assert_eq!(both.filter, "client | product");
    assert_eq!(
        plan(
            "client product",
            &synonyms(),
            &terms,
            &policy().with_max_ndoc(550)
        )
        .unwrap()
        .filter,
        both.filter
    );
    assert_eq!(
        plan(
            "client product",
            &synonyms(),
            &terms,
            &policy().with_max_ndoc(549)
        ),
        None
    );
}

#[test]
fn without_statistics_the_cap_never_applies() {
    let planned = plan(
        "catalogue import",
        &synonyms(),
        &SearchTerms::default(),
        &policy().with_max_ndoc(1),
    )
    .unwrap();
    assert_eq!(planned.filter, "(catalogue | catalog) | import");
}

#[test]
fn the_cap_survives_the_retry_policy() {
    let retry = SearchPolicy {
        retry_quorum: 0.34,
        ..policy().with_max_ndoc(5_000)
    }
    .retry()
    .unwrap();
    assert_eq!(retry.max_ndoc, 5_000);
}

#[test]
fn a_rare_enough_group_is_required_alone() {
    let rare_terms = terms(&[("akeneo", 1), ("data", 150), ("product", 154)], 2772);
    let hints = "What did we decide about Akeneo for product data";
    let plain = plan(hints, &synonyms(), &rare_terms, &policy()).unwrap();
    assert_eq!(plain.filter, "akeneo & data", "two of three, rarest first");
    let rare = plan(
        hints,
        &synonyms(),
        &rare_terms,
        &policy().with_rare_alone_share(0.001),
    )
    .unwrap();
    assert_eq!(
        rare.filter, "akeneo",
        "a word one fact carries is enough alone"
    );
    assert_eq!(rare.rank, plain.rank);
    let common = terms(&[("larchmont", 20), ("renew", 150), ("owns", 154)], 2772);
    let kept = plan(
        "When does Larchmont renew and who owns it",
        &synonyms(),
        &common,
        &policy().with_rare_alone_share(0.001),
    )
    .unwrap();
    assert_eq!(
        kept.filter, "larchmont & renew",
        "a client name is above the share"
    );
}
