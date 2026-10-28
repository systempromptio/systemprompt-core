//! Sharing guards: own stays with the owner, a share holder cannot lower a
//! higher grant, a non-member learns nothing, revocation ends presence and
//! events, expired grants are flagged and swept, and `document.shared` is
//! addressed per recipient.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use systemprompt_events::Broadcaster;
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::models::{GrantTarget, NewGrant};
use systemprompt_files::documents::service::{ShareRequest, TransferRequest};
use systemprompt_identifiers::{ConnectionId, DocumentId, UserId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    AccessLevel, EditOp, PresenceBeacon, PresenceSource, PresenceState,
};
use tempfile::TempDir;
use tokio::sync::mpsc;

use crate::documents_service_fixture::{
    GroupSource, Harness, RoleDirectory, allow, create, harness, harness_with, person, trace,
    unique,
};

async fn document(h: &Harness, owner: &AuthenticatedUser) -> DocumentId {
    let mut request = create("blank-text", None);
    request.ops = vec![EditOp::SetText {
        text: "body\n".to_owned(),
    }];
    h.service
        .create_from_template(owner, request, &trace())
        .await
        .expect("create")
        .0
        .id
}

fn grant(document: &DocumentId, kind: &str, value: &str, level: AccessLevel) -> ShareRequest {
    ShareRequest {
        target: GrantTarget::Document(document.clone()),
        subject_kind: kind.to_owned(),
        subject_value: value.to_owned(),
        subject_display: None,
        level,
        valid_until: None,
        message: None,
        via_agent: None,
    }
}

async fn legacy_own_grant(h: &Harness, document: &DocumentId, owner: &UserId, user: &UserId) {
    h.repos
        .grants
        .insert(NewGrant {
            target: GrantTarget::Document(document.clone()),
            subject_kind: "user".to_owned(),
            subject_value: user.as_str().to_owned(),
            subject_display: None,
            access_level: AccessLevel::Own,
            granted_by: owner.clone(),
            valid_until: None,
            trace_id: systemprompt_identifiers::TraceId::generate(),
            reason: None,
            via_agent: None,
        })
        .await
        .expect("insert own grant");
}

macro_rules! listen {
    ($h:expr, $user:expr) => {{
        let (tx, rx) = mpsc::channel(32);
        assert!(
            $h.broadcasters
                .agui
                .register($user, &ConnectionId::generate(), tx)
                .await
        );
        rx
    }};
}

async fn drain<T: std::fmt::Debug>(rx: &mut mpsc::Receiver<T>) -> String {
    let mut seen = String::new();
    while let Ok(Some(event)) = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
        seen.push_str(&format!("{event:?}"));
    }
    seen
}

#[tokio::test]
async fn a_share_holder_cannot_lower_a_grant_above_their_level() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("guard-owner");
    let sharer = person("guard-sharer");
    let senior = person("guard-senior");
    let id = document(&h, &owner).await;
    legacy_own_grant(&h, &id, &owner.id, &senior.id).await;
    h.service
        .share(
            &owner,
            grant(&id, "user", sharer.id.as_str(), AccessLevel::Share),
            &trace(),
        )
        .await
        .expect("owner shares");
    let refused = h
        .service
        .share(
            &sharer,
            grant(&id, "user", senior.id.as_str(), AccessLevel::View),
            &trace(),
        )
        .await
        .expect_err("cannot lower a higher grant");
    assert!(
        matches!(
            refused,
            DocumentsError::ShareBelowExisting {
                existing: AccessLevel::Own,
                held: AccessLevel::Share,
                ..
            }
        ),
        "{refused:?}"
    );
}

#[tokio::test]
async fn an_own_grant_holder_is_not_the_owner() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("own-owner");
    let holder = person("own-holder");
    let id = document(&h, &owner).await;
    legacy_own_grant(&h, &id, &owner.id, &holder.id).await;
    let view = h
        .service
        .get(&holder, &id, &trace())
        .await
        .expect("holder reads");
    assert_eq!(view.my_access, AccessLevel::Share);
    let transfer = h
        .service
        .transfer_ownership(
            &holder,
            &id,
            TransferRequest {
                to: holder.id.clone(),
                to_display: Some(holder.username.clone()),
            },
            &trace(),
        )
        .await
        .expect_err("holder cannot transfer");
    assert!(
        matches!(transfer, DocumentsError::Forbidden { .. }),
        "{transfer:?}"
    );
    let revoke_all = h
        .service
        .revoke_all(&holder, &id, &trace())
        .await
        .expect_err("holder cannot revoke every grant");
    assert!(
        matches!(revoke_all, DocumentsError::Forbidden { .. }),
        "{revoke_all:?}"
    );
    let trash = h
        .service
        .trash(&holder, &id, &trace())
        .await
        .expect_err("holder cannot trash");
    assert!(
        matches!(trash, DocumentsError::Forbidden { .. }),
        "{trash:?}"
    );
}

#[tokio::test]
async fn transfer_without_a_resolved_recipient_is_refused() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("transfer-owner");
    let id = document(&h, &owner).await;
    let refused = h
        .service
        .transfer_ownership(
            &owner,
            &id,
            TransferRequest {
                to: UserId::new(unique("ghost")),
                to_display: None,
            },
            &trace(),
        )
        .await
        .expect_err("unknown recipient");
    assert!(
        matches!(refused, DocumentsError::UnknownSubject { .. }),
        "{refused:?}"
    );
}

#[tokio::test]
async fn a_non_member_is_told_not_found_without_the_owner() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("probe-owner");
    let stranger = person("probe-stranger");
    let id = document(&h, &owner).await;
    let refused = h
        .service
        .get(&stranger, &id, &trace())
        .await
        .expect_err("probe");
    assert!(
        matches!(refused, DocumentsError::NotFound { .. }),
        "{refused:?}"
    );
    assert!(!refused.to_string().contains(owner.id.as_str()));
}

#[tokio::test]
async fn revoking_clears_presence_and_stops_events() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("revoke-owner");
    let reader = person("revoke-reader");
    let id = document(&h, &owner).await;
    let shared = h
        .service
        .share(
            &owner,
            grant(&id, "user", reader.id.as_str(), AccessLevel::View),
            &trace(),
        )
        .await
        .expect("share");
    h.service
        .touch_presence(
            &reader,
            PresenceBeacon {
                document_id: id.clone(),
                state: PresenceState::Viewing,
                source_kind: PresenceSource::Web,
                source_ref: unique("tab"),
                cursor: None,
            },
            &trace(),
        )
        .await
        .expect("reader present");
    let current = h
        .repos
        .documents
        .find(&id)
        .await
        .expect("find")
        .expect("document");
    assert!(h.service.recipients(&current).await.contains(&reader.id));
    h.service
        .revoke(
            &owner,
            &GrantTarget::Document(id.clone()),
            shared.id,
            &trace(),
        )
        .await
        .expect("revoke");
    let present = h.repos.presence.users_present(&id).await.expect("presence");
    assert!(!present.contains(&reader.id), "{present:?}");
    assert!(!h.service.recipients(&current).await.contains(&reader.id));
}

#[tokio::test]
async fn expired_grants_are_flagged_and_swept() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("expiry-owner");
    let reader = person("expiry-reader");
    let id = document(&h, &owner).await;
    let mut request = grant(&id, "user", reader.id.as_str(), AccessLevel::View);
    request.valid_until = Some(Utc::now() - chrono::Duration::minutes(1));
    let expired = h
        .service
        .share(&owner, request, &trace())
        .await
        .expect("share");
    let access = h
        .service
        .access_of(&owner, &id, false, &trace())
        .await
        .expect("access");
    let listed = access
        .grants
        .iter()
        .find(|g| g.id == expired.id)
        .expect("expired grant still listed");
    assert!(listed.is_expired(Utc::now()));
    h.repos.grants.sweep_expired(10_000).await.expect("sweep");
    let after = h
        .service
        .access_of(&owner, &id, false, &trace())
        .await
        .expect("access");
    assert!(after.grants.iter().all(|g| g.id != expired.id));
}

#[tokio::test]
async fn shared_events_reach_each_recipient_and_not_the_owner_of_a_co_share() {
    let owner = person("event-owner");
    let sharer = person("event-sharer");
    let alice = person("event-alice");
    let bob = person("event-bob");
    let role = unique("team");
    let directory = RoleDirectory::default().with_role(&role, &[&alice.id, &bob.id]);
    let h = harness_with(
        allow(),
        GroupSource::new(&[]),
        Arc::new(TempDir::new().expect("tempdir")),
        directory,
    )
    .await;
    let id = document(&h, &owner).await;
    h.service
        .share(
            &owner,
            grant(&id, "user", sharer.id.as_str(), AccessLevel::Share),
            &trace(),
        )
        .await
        .expect("owner shares");
    let mut owner_rx = listen!(h, &owner.id);
    let mut alice_rx = listen!(h, &alice.id);
    let mut bob_rx = listen!(h, &bob.id);
    h.service
        .share(
            &sharer,
            grant(&id, "role", &role, AccessLevel::View),
            &trace(),
        )
        .await
        .expect("co-sharer shares with the role");
    let alice_seen = drain(&mut alice_rx).await;
    let bob_seen = drain(&mut bob_rx).await;
    assert!(alice_seen.contains("document.shared"), "{alice_seen}");
    assert!(alice_seen.contains(alice.id.as_str()), "{alice_seen}");
    assert!(!alice_seen.contains(bob.id.as_str()), "{alice_seen}");
    assert!(bob_seen.contains(bob.id.as_str()), "{bob_seen}");
    let owner_seen = drain(&mut owner_rx).await;
    assert!(!owner_seen.contains("document.shared"), "{owner_seen}");
}
