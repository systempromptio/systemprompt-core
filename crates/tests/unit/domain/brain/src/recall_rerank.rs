use systemprompt_brain::recall::rerank::{Reranked, merge, record, returned_of};
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::ScoredFact;

use crate::recall_fixtures::{brain_fact, id, scored};

fn visible() -> Vec<ScoredFact> {
    (1..=4)
        .map(|n| scored(brain_fact(n, "ownership"), 1.0 / f64::from(n)))
        .collect()
}

fn ids(facts: &[ScoredFact]) -> Vec<BrainFactId> {
    facts.iter().map(|f| f.fact.id.clone()).collect()
}

#[test]
fn returned_ids_come_first_in_the_returned_order_and_the_rest_keep_fused_order() {
    let merged = merge(visible(), &[id(3), id(1)]);
    assert_eq!(ids(&merged), vec![id(3), id(1), id(2), id(4)]);
}

#[test]
fn an_unknown_id_is_ignored_and_a_repeat_counts_at_its_first_position() {
    let merged = merge(visible(), &[id(9), id(4), id(4), id(2)]);
    assert_eq!(ids(&merged), vec![id(4), id(2), id(1), id(3)]);
    assert_eq!(
        returned_of(&visible(), &[id(9), id(4), id(4), id(2)]),
        vec![id(4), id(2)]
    );
}

#[test]
fn an_empty_or_all_unknown_result_leaves_the_order_unchanged() {
    assert_eq!(ids(&merge(visible(), &[])), ids(&visible()));
    assert_eq!(ids(&merge(visible(), &[id(7)])), ids(&visible()));
    assert!(returned_of(&visible(), &[id(7)]).is_empty());
}

#[test]
fn the_record_names_the_reranker_and_ranks_only_the_injected_facts_it_returned() {
    let reranked = Reranked {
        reranker: "test.reverse",
        order: vec![id(3), id(1)],
    };
    let injected = merge(visible(), &reranked.order);
    let rerank = record(&reranked, &injected[..3]);
    assert_eq!(rerank.reranker, "test.reverse");
    let ranks: Vec<(BrainFactId, u32)> = rerank
        .ranks
        .iter()
        .map(|r| (r.id.clone(), r.rank))
        .collect();
    assert_eq!(ranks, vec![(id(3), 1), (id(1), 2)], "id 2 was omitted");
}
