//! Access requests: the config gate, one open request per user and
//! document, the owner's event, and granting or declining from `share`.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_events::Broadcaster;
use systemprompt_files::DocumentsConfig;
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::service::{AccessDecision, AccessRequestInput, DecideRequest};
use systemprompt_identifiers::{ConnectionId, DocumentId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AccessLevel, AccessRequestStatus, EditOp, GrantEventKind};
use tempfile::TempDir;
use tokio::sync::mpsc;

use crate::documents_service_fixture::{
    GroupSource, Harness, RoleDirectory, allow, create, harness, harness_configured, person, trace,
};

async fn requests_on() -> Harness {
    let config = DocumentsConfig {
        access_requests: true,
        ..DocumentsConfig::default()
    };
    harness_configured(
        allow(),
        GroupSource::new(&[]),
        Arc::new(TempDir::new().expect("tempdir")),
        RoleDirectory::default(),
        config,
    )
    .await
}

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

fn asking(level: AccessLevel) -> AccessRequestInput {
    AccessRequestInput {
        level,
        message: Some("need this for the review".to_owned()),
    }
}

#[tokio::test]
async fn a_request_reaches_the_owner_once_and_granting_shares_the_level() {
    let h = requests_on().await;
    let owner = person("request-owner");
    let asker = person("request-asker");
    let id = document(&h, &owner).await;
    assert!(h.service.can_request_access(&asker, &id).await);
    assert!(!h.service.can_request_access(&owner, &id).await);
    let (tx, mut rx) = mpsc::channel(32);
    assert!(
        h.broadcasters
            .agui
            .register(&owner.id, &ConnectionId::generate(), tx)
            .await
    );
    let request = h
        .service
        .request_access(&asker, &id, asking(AccessLevel::Comment))
        .await
        .expect("request");
    assert_eq!(request.status, AccessRequestStatus::Open);
    let mut seen = String::new();
    while let Ok(Some(event)) = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
        seen.push_str(&format!("{event:?}"));
    }
    assert!(
        seen.contains("document.access_requested") && seen.contains("need this"),
        "{seen}"
    );
    let again = h
        .service
        .request_access(&asker, &id, asking(AccessLevel::View))
        .await
        .expect_err("one open request");
    assert!(
        matches!(again, DocumentsError::AccessRequestOpen { .. }),
        "{again:?}"
    );
    let listed = h
        .service
        .access_of(&owner, &id, false, &trace())
        .await
        .expect("access");
    assert_eq!(listed.requests.len(), 1);
    let decided = h
        .service
        .decide_access_request(
            &owner,
            DecideRequest {
                document: id.clone(),
                request: request.id.clone(),
                decision: AccessDecision::Grant {
                    subject_display: Some("Asker".to_owned()),
                },
            },
            &trace(),
        )
        .await
        .expect("grant");
    assert_eq!(decided.status, AccessRequestStatus::Granted);
    assert_eq!(decided.decided_by.as_ref(), Some(&owner.id));
    let mine = h
        .service
        .access_of(&asker, &id, false, &trace())
        .await
        .expect("asker access");
    assert_eq!(mine.my_access, AccessLevel::Comment);
    let history = h
        .service
        .access_of(&owner, &id, true, &trace())
        .await
        .expect("history");
    assert_eq!(history.history[0].event, GrantEventKind::Granted);
    assert!(history.requests.is_empty());
    let closed = h
        .service
        .decide_access_request(
            &owner,
            DecideRequest {
                document: id.clone(),
                request: request.id,
                decision: AccessDecision::Decline,
            },
            &trace(),
        )
        .await
        .expect_err("already granted");
    assert!(matches!(closed, DocumentsError::AccessRequestClosed { .. }));
    let needless = h
        .service
        .request_access(&asker, &id, asking(AccessLevel::Edit))
        .await
        .expect_err("already holds a level");
    assert!(matches!(
        needless,
        DocumentsError::AccessRequestNotNeeded {
            held: AccessLevel::Comment,
            ..
        }
    ));
}

#[tokio::test]
async fn declining_closes_the_request_and_a_new_one_may_follow() {
    let h = requests_on().await;
    let owner = person("decline-owner");
    let asker = person("decline-asker");
    let stranger = person("decline-stranger");
    let id = document(&h, &owner).await;
    let request = h
        .service
        .request_access(&asker, &id, asking(AccessLevel::View))
        .await
        .expect("request");
    let refused = h
        .service
        .decide_access_request(
            &stranger,
            DecideRequest {
                document: id.clone(),
                request: request.id.clone(),
                decision: AccessDecision::Decline,
            },
            &trace(),
        )
        .await
        .expect_err("a stranger cannot answer");
    assert!(
        matches!(refused, DocumentsError::NotFound { .. }),
        "{refused:?}"
    );
    let declined = h
        .service
        .decide_access_request(
            &owner,
            DecideRequest {
                document: id.clone(),
                request: request.id,
                decision: AccessDecision::Decline,
            },
            &trace(),
        )
        .await
        .expect("decline");
    assert_eq!(declined.status, AccessRequestStatus::Declined);
    let still_none = h.service.access_of(&asker, &id, false, &trace()).await;
    assert!(matches!(still_none, Err(DocumentsError::NotFound { .. })));
    h.service
        .request_access(&asker, &id, asking(AccessLevel::View))
        .await
        .expect("a new request after a decline");
}

#[tokio::test]
async fn requests_are_refused_as_not_found_while_the_install_disables_them() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("gate-owner");
    let asker = person("gate-asker");
    let id = document(&h, &owner).await;
    assert!(!h.service.can_request_access(&asker, &id).await);
    let refused = h
        .service
        .request_access(&asker, &id, asking(AccessLevel::View))
        .await
        .expect_err("disabled");
    assert!(
        matches!(refused, DocumentsError::NotFound { .. }),
        "{refused:?}"
    );
    let on = requests_on().await;
    let missing = DocumentId::generate();
    assert!(!on.service.can_request_access(&asker, &missing).await);
}
