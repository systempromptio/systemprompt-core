use std::sync::Arc;

use systemprompt_files::documents::collab::{CollabService, CollabTuning, JoinMode};

use crate::documents_collab::{Collab, client, collab, edit, html_document, loro, request};
use crate::documents_media_admit::uploads_beside;
use crate::documents_media_provenance::{distinct_picture, upload};
use crate::documents_service_fixture::{person, trace};

async fn with_uploads(c: &Collab) -> Arc<CollabService> {
    Arc::new(
        CollabService::new(Arc::clone(&c.documents), CollabTuning::default(), loro())
            .with_uploads(Arc::new(uploads_beside(&c.h).await)),
    )
}

async fn place(
    c: &Collab,
    collab: &CollabService,
    who: &systemprompt_models::auth::AuthenticatedUser,
    hash: &str,
) -> systemprompt_identifiers::DocumentId {
    let document = html_document(c, who).await;
    let joined = collab
        .join(who, &document, JoinMode::Edit, &trace())
        .await
        .expect("join");
    let doc = client(&joined);
    let update = edit(
        &doc,
        &format!("<p>Picture</p><img src=\"{hash}\" alt=\"p\">"),
    );
    collab
        .apply(who, request(&document, &joined, &doc, update), &trace())
        .await
        .expect("apply");
    document
}

#[tokio::test]
async fn a_live_image_is_served_only_to_a_session_whose_inserter_uploaded_it() {
    let c = collab().await;
    let collab = with_uploads(&c).await;
    let alice = person("live-image-owner");
    let bob = person("live-image-namer");
    let picture = distinct_picture();
    let hash = upload(&c.h, &alice, &picture).await;

    let hers = place(&c, &collab, &alice, &hash).await;
    let served = collab
        .live_image(&alice, &hers, &hash, &trace())
        .await
        .expect("the uploader's own live image is served");
    assert_eq!(served.bytes, picture);

    let his = place(&c, &collab, &bob, &hash).await;
    assert!(
        collab
            .live_image(&bob, &his, &hash, &trace())
            .await
            .is_none(),
        "naming another user's hash in a live document serves nothing"
    );
}
