use systemprompt_brain::recall::fusion::TierHits;
use systemprompt_brain::recall::introduce::{extension_hits, known, unseen};
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::RetrievalTier;

use crate::recall_fixtures::{brain_fact, id};

fn core() -> Vec<TierHits> {
    vec![
        TierHits::new(RetrievalTier::Subject, vec![brain_fact(1, "a")]),
        TierHits::new(
            RetrievalTier::FullText,
            vec![brain_fact(2, "a"), brain_fact(1, "a")],
        ),
    ]
}

#[test]
fn unseen_ids_are_taken_round_robin_by_rank_deduplicated_and_capped() {
    let known = known(&core());
    assert_eq!(known.len(), 2);
    let ranked = vec![
        vec![id(1), id(7), id(8), id(9)],
        vec![id(5), id(7), id(2), id(6)],
    ];
    assert_eq!(
        unseen(&known, &ranked, 24),
        vec![id(5), id(7), id(8), id(9), id(6)]
    );
    assert_eq!(unseen(&known, &ranked, 2), vec![id(5), id(7)]);
    assert_eq!(unseen(&known, &[], 24), Vec::<BrainFactId>::new());
}

#[test]
fn extension_hits_keep_each_retriever_order_and_skip_what_was_not_loaded() {
    let mut known = known(&core());
    known.insert(id(7), brain_fact(7, "a"));
    let ranked = vec![vec![id(7), id(1), id(9)], vec![id(2)]];
    let hits = extension_hits(&ranked, &known);
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].tier, RetrievalTier::Extension);
    let first: Vec<_> = hits[0].facts.iter().map(|f| f.id.clone()).collect();
    assert_eq!(first, vec![id(7), id(1)], "id 9 was never loaded");
    let second: Vec<_> = hits[1].facts.iter().map(|f| f.id.clone()).collect();
    assert_eq!(second, vec![id(2)]);
}
