use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::models::{GrantTarget, NewGrant};
use systemprompt_files::documents::service::ShareRequest;
use systemprompt_identifiers::{DocumentId, TraceId, UserId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AccessLevel, EditOp};
use systemprompt_test_fixtures::test_db_pool;
use uuid::Uuid;

use crate::documents_service_fixture::{
    GroupSource, Harness, allow, create, harness, person, trace,
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

fn grant(document: &DocumentId, user: &UserId, level: AccessLevel) -> ShareRequest {
    ShareRequest {
        target: GrantTarget::Document(document.clone()),
        subject_kind: "user".to_owned(),
        subject_value: user.as_str().to_owned(),
        subject_display: None,
        level,
        valid_until: None,
        message: None,
        via_agent: None,
    }
}

async fn own_grant(h: &Harness, document: &DocumentId, owner: &UserId, user: &UserId) {
    let pool = test_db_pool().await;
    let mut conn = pool.write_pool().acquire().await.expect("acquire");
    h.repos
        .grants
        .insert(
            &mut conn,
            NewGrant {
                target: GrantTarget::Document(document.clone()),
                subject_kind: "user".to_owned(),
                subject_value: user.as_str().to_owned(),
                subject_display: None,
                access_level: AccessLevel::Own,
                granted_by: owner.clone(),
                valid_until: None,
                trace_id: TraceId::generate(),
                reason: None,
                via_agent: None,
            },
        )
        .await
        .expect("insert own grant");
}

#[tokio::test]
async fn grants_list_what_was_shared_and_revoke_removes_it() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("revoke-list-owner");
    let reader = person("revoke-list-reader");
    let id = document(&h, &owner).await;
    let target = GrantTarget::Document(id.clone());
    let shared = h
        .service
        .share(&owner, grant(&id, &reader.id, AccessLevel::View), &trace())
        .await
        .expect("share");
    let listed = h
        .service
        .grants(&owner, &target, &trace())
        .await
        .expect("grants");
    assert!(
        listed
            .iter()
            .any(|g| g.id == shared.id && g.access_level == AccessLevel::View)
    );

    h.service
        .revoke(&owner, &target, shared.id, &trace())
        .await
        .expect("revoke");
    let after = h
        .service
        .grants(&owner, &target, &trace())
        .await
        .expect("grants");
    assert!(after.iter().all(|g| g.id != shared.id));

    let again = h
        .service
        .revoke(&owner, &target, shared.id, &trace())
        .await
        .expect_err("already revoked");
    assert!(
        matches!(again, DocumentsError::NotFound { .. }),
        "{again:?}"
    );
}

#[tokio::test]
async fn a_share_holder_cannot_revoke_a_grant_above_their_level() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("revoke-up-owner");
    let sharer = person("revoke-up-sharer");
    let senior = person("revoke-up-senior");
    let id = document(&h, &owner).await;
    let target = GrantTarget::Document(id.clone());
    own_grant(&h, &id, &owner.id, &senior.id).await;
    h.service
        .share(&owner, grant(&id, &sharer.id, AccessLevel::Share), &trace())
        .await
        .expect("share");
    let senior_grant = h
        .service
        .grants(&sharer, &target, &trace())
        .await
        .expect("grants")
        .into_iter()
        .find(|g| g.subject_value == senior.id.as_str())
        .expect("senior grant");
    let refused = h
        .service
        .revoke(&sharer, &target, senior_grant.id, &trace())
        .await
        .expect_err("above level");
    assert!(
        matches!(
            refused,
            DocumentsError::Forbidden {
                needed: AccessLevel::Own,
                held: Some(AccessLevel::Share),
                ..
            }
        ),
        "{refused:?}"
    );
    let unknown = h
        .service
        .revoke(&sharer, &target, Uuid::new_v4(), &trace())
        .await
        .expect_err("unknown grant");
    assert!(
        matches!(unknown, DocumentsError::NotFound { .. }),
        "{unknown:?}"
    );
}
