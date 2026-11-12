use std::sync::Arc;

use systemprompt_files::documents::models::NewFolder;
use systemprompt_governance::authz::{
    AuthzDecision, AuthzDecisionHook, AuthzRequest, DenyReason, EntityRef, SharedAuthzHook,
};
use systemprompt_identifiers::{DocumentFolderId, DocumentId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::EditOp;

use crate::documents_service_fixture::{
    GroupSource, Harness, allow, create, harness, person, trace, unique,
};

#[derive(Debug)]
struct FolderDenyHook {
    closed: DocumentFolderId,
}

#[async_trait::async_trait]
impl AuthzDecisionHook for FolderDenyHook {
    async fn evaluate(&self, req: AuthzRequest) -> AuthzDecision {
        self.probe(req).await
    }

    async fn probe(&self, req: AuthzRequest) -> AuthzDecision {
        match &req.entity {
            EntityRef::Folder(id) if *id == self.closed => AuthzDecision::Deny {
                reason: DenyReason::HookUnavailable {
                    policy: "folder-deny".to_owned(),
                    detail: "closed folder".to_owned(),
                },
                policy: "folder-deny".to_owned(),
            },
            _ => AuthzDecision::Allow,
        }
    }
}

async fn written(
    h: &Harness,
    who: &AuthenticatedUser,
    folder: Option<&DocumentFolderId>,
    text: String,
) -> DocumentId {
    let mut request = create("blank-text", folder);
    request.name = unique("found");
    request.ops = vec![EditOp::SetText { text }];
    h.service
        .create_from_template(who, request, &trace())
        .await
        .expect("create")
        .0
        .id
}

#[tokio::test]
async fn hidden_top_ranked_hits_do_not_crowd_out_visible_matches() {
    let setup = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("search-crowd");
    let word = unique("quokka").replace('-', "");
    let closed = setup
        .repos
        .folders
        .insert(NewFolder {
            parent_id: None,
            name: unique("Closed"),
            owner_user_id: owner.id.clone(),
            created_by: owner.id.clone(),
        })
        .await
        .expect("folder")
        .id;
    let mut hidden = Vec::new();
    for _ in 0..6 {
        let text = format!("{word} {word} {word} {word}\n");
        hidden.push(written(&setup, &owner, Some(&closed), text).await);
    }
    let visible = written(&setup, &owner, None, format!("one {word} only\n")).await;

    let h = harness(
        Arc::new(FolderDenyHook { closed }) as SharedAuthzHook,
        GroupSource::new(&[]),
    )
    .await;
    let found = h
        .service
        .search(&owner, &word, 3, &trace())
        .await
        .expect("search");
    let ids: Vec<DocumentId> = found.iter().map(|r| r.document.id.clone()).collect();
    assert_eq!(ids, vec![visible]);
    assert!(found.iter().all(|r| r.hit.document_id == r.document.id));
    assert!(ids.iter().all(|id| !hidden.contains(id)));
}
