//! The `document.drive.changed` hint a landed revision sends: who receives
//! it (grant subjects on the document and its folder chain, never a stranger
//! or a revoked user) and how revisions inside one window coalesce.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_events::Broadcaster;
use systemprompt_files::documents::model::Media;
use systemprompt_files::documents::models::GrantTarget;
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{
    CommentPolicy, CommitOutcome, CommitRequest, DRIVE_HINT_INTERVAL, NewFolderRequest,
    ShareRequest,
};
use systemprompt_identifiers::{ConnectionId, DocumentFolderId, DocumentId, DocumentRevisionId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AccessLevel, DocumentDriveChangedEvent, EditOp};
use tempfile::TempDir;
use tokio::sync::mpsc;

use crate::documents_service_fixture::{
    GroupSource, Harness, RoleDirectory, allow, api_author, create, harness, harness_with, person,
    trace, unique,
};

async fn listen(h: &Harness, who: &AuthenticatedUser) -> mpsc::Receiver<impl std::fmt::Debug> {
    let (tx, rx) = mpsc::channel(64);
    assert!(
        h.broadcasters
            .agui
            .register(&who.id, &ConnectionId::generate(), tx)
            .await
    );
    rx
}

async fn hints_within<T: std::fmt::Debug>(rx: &mut mpsc::Receiver<T>, window: Duration) -> usize {
    let deadline = tokio::time::Instant::now() + window;
    let mut hints = 0;
    while let Ok(Some(frame)) = tokio::time::timeout_at(deadline, rx.recv()).await {
        if format!("{frame:?}").contains(DocumentDriveChangedEvent::NAME) {
            hints += 1;
        }
    }
    hints
}

async fn document(
    h: &Harness,
    owner: &AuthenticatedUser,
    folder: Option<&DocumentFolderId>,
) -> (DocumentId, DocumentRevisionId) {
    let mut request = create("blank-text", folder);
    request.ops = vec![EditOp::SetText {
        text: "body\n".to_owned(),
    }];
    let (document, revision) = h
        .service
        .create_from_template(owner, request, &trace())
        .await
        .expect("create");
    (document.id, revision.id)
}

async fn commit(
    h: &Harness,
    who: &AuthenticatedUser,
    document: &DocumentId,
    base: &DocumentRevisionId,
    text: &str,
) -> DocumentRevisionId {
    let outcome = h
        .service
        .commit(
            who,
            CommitRequest {
                document_id: document.clone(),
                base_revision: Some(base.clone()),
                body: CommitBody::Bytes(text.as_bytes().to_vec()),
                author: api_author(),
                message: None,
                lock_holder: None,
                comments: CommentPolicy::default(),
                op_id: None,
                media: Media::new(),
            },
            &trace(),
        )
        .await
        .expect("commit");
    let CommitOutcome::Committed(landed) = outcome else {
        panic!("expected Committed, got {outcome:?}");
    };
    landed.revision.id
}

fn share(target: GrantTarget, kind: &str, value: &str, level: AccessLevel) -> ShareRequest {
    ShareRequest {
        target,
        subject_kind: kind.to_owned(),
        subject_value: value.to_owned(),
        subject_display: None,
        level,
        valid_until: None,
        message: None,
        via_agent: None,
    }
}

async fn quiet() {
    tokio::time::sleep(DRIVE_HINT_INTERVAL + Duration::from_millis(300)).await;
}

#[tokio::test]
async fn a_revision_hints_every_access_holder_and_no_stranger() {
    let reader = person("hint-reader");
    let member = person("hint-member");
    let inheritor = person("hint-inheritor");
    let stranger = person("hint-stranger");
    let owner = person("hint-owner");
    let role = unique("hint-role");
    let h = harness_with(
        allow(),
        GroupSource::new(&[]),
        Arc::new(TempDir::new().expect("tempdir")),
        RoleDirectory::default().with_role(&role, &[&member.id]),
    )
    .await;
    let folder = h
        .service
        .create_folder(
            &owner,
            NewFolderRequest {
                parent: None,
                name: unique("Hinted"),
            },
            &trace(),
        )
        .await
        .expect("folder")
        .id;
    let (id, first) = document(&h, &owner, Some(&folder)).await;
    let document_target = GrantTarget::Document(id.clone());
    for request in [
        share(
            document_target.clone(),
            "user",
            reader.id.as_str(),
            AccessLevel::View,
        ),
        share(document_target, "role", &role, AccessLevel::Comment),
        share(
            GrantTarget::Folder(folder),
            "user",
            inheritor.id.as_str(),
            AccessLevel::View,
        ),
    ] {
        h.service
            .share(&owner, request, &trace())
            .await
            .expect("share");
    }
    quiet().await;
    let mut owner_rx = listen(&h, &owner).await;
    let mut reader_rx = listen(&h, &reader).await;
    let mut member_rx = listen(&h, &member).await;
    let mut inheritor_rx = listen(&h, &inheritor).await;
    let mut stranger_rx = listen(&h, &stranger).await;

    commit(&h, &owner, &id, &first, "two\n").await;
    let window = Duration::from_secs(1);
    assert_eq!(hints_within(&mut owner_rx, window).await, 1, "the owner");
    assert_eq!(
        hints_within(&mut reader_rx, window).await,
        1,
        "a user grant"
    );
    assert_eq!(
        hints_within(&mut member_rx, window).await,
        1,
        "a role grant"
    );
    assert_eq!(
        hints_within(&mut inheritor_rx, window).await,
        1,
        "a grant on the folder chain"
    );
    assert_eq!(
        hints_within(&mut stranger_rx, window).await,
        0,
        "a user with no access is never hinted"
    );
}

#[tokio::test]
async fn a_revoked_user_is_not_hinted() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("hint-revoke-owner");
    let former = person("hint-revoke-former");
    let (id, first) = document(&h, &owner, None).await;
    let target = GrantTarget::Document(id.clone());
    let granted = h
        .service
        .share(
            &owner,
            share(
                target.clone(),
                "user",
                former.id.as_str(),
                AccessLevel::Edit,
            ),
            &trace(),
        )
        .await
        .expect("share");
    h.service
        .revoke(&owner, &target, granted.id, &trace())
        .await
        .expect("revoke");
    quiet().await;
    let mut owner_rx = listen(&h, &owner).await;
    let mut former_rx = listen(&h, &former).await;

    commit(&h, &owner, &id, &first, "after revoke\n").await;
    let window = Duration::from_secs(1);
    assert_eq!(hints_within(&mut owner_rx, window).await, 1);
    assert_eq!(
        hints_within(&mut former_rx, window).await,
        0,
        "a revoked grant no longer reaches its user"
    );
}

#[tokio::test]
async fn revisions_inside_one_window_coalesce_into_one_trailing_hint() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("hint-burst-owner");
    let mut owner_rx = listen(&h, &owner).await;
    let (id, mut head) = document(&h, &owner, None).await;
    for step in 0..3 {
        head = commit(&h, &owner, &id, &head, &format!("step {step}\n")).await;
    }

    let hints = hints_within(
        &mut owner_rx,
        DRIVE_HINT_INTERVAL + Duration::from_millis(1500),
    )
    .await;
    assert_eq!(
        hints, 2,
        "the first revision is hinted at once and the rest of the window once"
    );
    assert_eq!(
        hints_within(&mut owner_rx, Duration::from_millis(500)).await,
        0,
        "nothing more once the window has drained"
    );
}
