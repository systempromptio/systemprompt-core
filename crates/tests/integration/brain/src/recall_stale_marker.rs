use systemprompt_brain::recall::STALE_MARKER;
use systemprompt_extension::brain::{MemoryRecall, RecallConversation, RecallRequest, ReuseHint};
use systemprompt_manifest::profile::StalePolicy;

use crate::harness::{Harness, Setup};
use crate::recall_seed::{HINTS, PROJECT, seed};
use crate::recall_support::{ids, injected, principal};

#[tokio::test]
async fn a_marker_policy_injects_a_stale_fact_with_its_marker_under_the_profile_budget() {
    let mut setup = Setup::confirming();
    setup.profile.recall.stale = StalePolicy::Marker;
    let h = Harness::with("brain_recall_stale_marker", setup).await;
    let seeded = seed(&h).await;
    let stale = seeded.excluded[0].clone();

    let request = RecallRequest::new(
        principal(&seeded.reader, Some(PROJECT)),
        RecallConversation::default(),
    )
    .with_subject_hints(HINTS)
    .with_reuse(ReuseHint::NewTurn);
    let outcome = h.recall().recall(&request).await.expect("recall");

    let got = ids(&outcome);
    assert!(got.contains(&stale), "{got:?}");
    assert!(!got.contains(&seeded.excluded[1]), "retired stays out");
    let block = injected(&outcome).expect("block").render();
    assert!(block.contains(STALE_MARKER.trim()), "{block}");
    h.finish().await;
}
