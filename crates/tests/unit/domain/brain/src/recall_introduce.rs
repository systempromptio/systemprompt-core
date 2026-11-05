use std::collections::HashSet;

use systemprompt_brain::recall::introduce::{extension_hits, known, unseen};
use systemprompt_extension::brain::TierIds;
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::RetrievalTier;

use crate::recall_fixtures::id;

fn core() -> Vec<TierIds> {
    vec![
        TierIds::new(RetrievalTier::Subject, vec![id(1)]),
        TierIds::new(RetrievalTier::FullText, vec![id(2), id(1)]),
    ]
}

#[test]
fn unseen_ids_are_taken_round_robin_by_rank_deduplicated_and_capped() {
    let known = known(&core());
    assert_eq!(known, HashSet::from([id(1), id(2)]));
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
fn extension_hits_keep_each_retriever_order_and_skip_what_is_not_known() {
    let mut known = known(&core());
    known.insert(id(7));
    let ranked = vec![vec![id(7), id(1), id(9)], vec![id(2)]];
    let hits = extension_hits(&ranked, &known);
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].tier, RetrievalTier::Extension);
    assert_eq!(
        hits[0].ids,
        vec![id(7), id(1)],
        "id 9 was never loaded, so it takes no rank"
    );
    assert_eq!(hits[1].ids, vec![id(2)]);
}
