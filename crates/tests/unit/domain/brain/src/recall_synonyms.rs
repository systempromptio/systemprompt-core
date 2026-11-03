use systemprompt_brain::recall::synonyms::{expand, search_query};
use systemprompt_brain::recall::tiers;
use systemprompt_models::brain::SynonymsDoc;

fn doc() -> SynonymsDoc {
    SynonymsDoc {
        groups: vec![
            vec![
                "customer".to_owned(),
                "client".to_owned(),
                "account".to_owned(),
            ],
            vec!["Invoice".to_owned(), "bill".to_owned()],
        ],
    }
}

#[test]
fn a_query_word_in_a_group_expands_to_the_whole_group_once() {
    assert_eq!(
        search_query("the client pays the CLIENT invoice", &doc()),
        "the or customer or client or account or pays or invoice or bill"
    );
    assert_eq!(expand(&doc(), "bill"), vec!["Invoice", "bill"]);
    assert_eq!(expand(&doc(), "ledger"), vec!["ledger"]);
}

#[test]
fn without_groups_the_query_is_the_distinct_words_of_three_or_more_characters() {
    assert_eq!(
        tiers::search_query("Acme, acme & co: a b payments"),
        "acme or payments"
    );
    assert_eq!(search_query("", &doc()), "");
}

#[test]
fn the_query_is_capped_at_thirty_two_words_after_expansion() {
    let hints: String = (0..40).map(|n| format!("word{n:02} ")).collect();
    let query = search_query(&hints, &doc());
    assert_eq!(query.split(" or ").count(), 32);
    let expanded = search_query(&format!("client {hints}"), &doc());
    assert_eq!(expanded.split(" or ").count(), 32);
    assert!(expanded.starts_with("customer or client or account or word00"));
}
