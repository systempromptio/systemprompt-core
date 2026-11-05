use std::collections::HashMap;

use systemprompt_brain::recall::tiers::matched::{MAX_NGRAM_WORDS, ngrams, probes};
use systemprompt_brain::repository::SearchTerms;

fn terms(pairs: &[(&str, u64)]) -> SearchTerms {
    SearchTerms {
        total: 1000,
        ndoc: pairs
            .iter()
            .map(|(w, n)| ((*w).to_owned(), *n))
            .collect::<HashMap<_, _>>(),
    }
}

#[test]
fn every_run_of_up_to_four_words_is_an_ngram_once() {
    let grams = ngrams("larchmont platform renewal for larchmont platform");
    assert_eq!(MAX_NGRAM_WORDS, 4);
    for expected in [
        "larchmont",
        "larchmont platform",
        "larchmont platform renewal",
        "larchmont platform renewal for",
        "platform renewal for larchmont",
        "renewal for larchmont platform",
        "platform",
    ] {
        assert!(
            grams.contains(&expected.to_owned()),
            "{expected:?} in {grams:?}"
        );
    }
    assert!(!grams.iter().any(|g| g.split(' ').count() > MAX_NGRAM_WORDS));
    assert_eq!(
        grams
            .iter()
            .filter(|g| g.as_str() == "larchmont platform")
            .count(),
        1,
        "deduplicated"
    );
    assert_eq!(ngrams(""), Vec::<String>::new());
}

#[test]
fn punctuation_at_a_word_edge_is_trimmed_and_tokens_are_ngrams_too() {
    let grams = ngrams("acme's payments: the e-commerce hub, (sap) and north star.");
    for expected in [
        "acme",
        "acme's",
        "e-commerce hub",
        "e-commerce",
        "commerce hub",
        "hub",
        "sap",
        "north star",
        "payments",
    ] {
        assert!(
            grams.contains(&expected.to_owned()),
            "{expected:?} in {grams:?}"
        );
    }
    assert!(!grams.contains(&"hub,".to_owned()));
    assert!(!grams.contains(&"(sap)".to_owned()));
}

#[test]
fn probes_are_the_unknown_tokens_of_three_or_more_characters() {
    let hints = "is caloway on the e-commerce platform? caloway is";
    let all = probes(hints, &SearchTerms::default());
    assert_eq!(
        all,
        ["caloway", "the", "commerce", "platform"],
        "without statistics every token probes, once"
    );
    let known = probes(hints, &terms(&[("platform", 40), ("the", 900)]));
    assert_eq!(known, ["caloway", "commerce"]);
    assert_eq!(probes("", &SearchTerms::default()), Vec::<String>::new());
}
