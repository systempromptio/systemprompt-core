use std::sync::Arc;
use std::time::Duration;

use systemprompt_files::documents::collab::{CollabService, CollabTuning, JoinMode};
use systemprompt_files::documents::engine::loro::rich;
use systemprompt_files::documents::model::BlockKind;

use crate::documents_collab::{Collab, client, collab, edit, html_document, loro, request};
use crate::documents_service_fixture::{person, trace};

const UNOWNED: &str = "0f9c1b1c3f0e2a7d6b5c4a39281706f5e4d3c2b1a0918273645546372819a0b1";

fn checkpointing(c: &Collab) -> Arc<CollabService> {
    Arc::new(CollabService::new(
        Arc::clone(&c.documents),
        CollabTuning {
            snapshot_after: Duration::from_secs(600),
            snapshot_after_updates: 10_000,
            checkpoint_after: Duration::ZERO,
            tick: Duration::from_millis(10),
            evict_after: Duration::from_secs(600),
        },
        loro(),
    ))
}

#[tokio::test]
async fn a_degraded_checkpoint_saves_a_placeholder_and_leaves_the_live_image() {
    let c = collab().await;
    let collab = checkpointing(&c);
    let owner = person("collab-image");
    let document = html_document(&c, &owner).await;
    let joined = collab
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("join");
    let doc = client(&joined);
    let update = edit(
        &doc,
        &format!("<p>Chart</p><img src=\"{UNOWNED}\" alt=\"chart\">"),
    );
    collab
        .apply(&owner, request(&document, &joined, &doc, update), &trace())
        .await
        .expect("apply");
    collab.tick().await;
    let (_, bytes) = c
        .documents
        .read_bytes(&owner, &document, None, &trace())
        .await
        .expect("read");
    let saved = String::from_utf8(bytes).expect("utf8");
    assert!(
        saved.contains("[image not available]"),
        "the saved bytes degrade"
    );
    assert!(!saved.contains(UNOWNED), "the unowned image is not saved");
    let again = collab
        .join(&owner, &document, JoinMode::View, &trace())
        .await
        .expect("rejoin");
    let live = rich::read(&client(&again)).expect("read live");
    assert!(
        live.blocks.iter().any(|block| matches!(
            &block.kind,
            BlockKind::Image { blob, .. } if blob == UNOWNED
        )),
        "the live document keeps the image"
    );
}
