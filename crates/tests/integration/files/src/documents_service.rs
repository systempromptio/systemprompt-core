use std::time::Duration;

use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::models::{
    GrantTarget, ListScope, ListSort, LockHolderKind, LockOutcome,
};
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{
    AcquireLockRequest, CommentPolicy, CommitOutcome, CommitRequest, HistoryPage, ImportRequest,
    LandedCommit, ListFolderRequest, NewFolderRequest, ShareRequest,
};
use systemprompt_governance::authz::EntityKind;
use systemprompt_identifiers::{DocumentId, DocumentNodeId, DocumentRevisionId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AccessLevel, ContentAs, DocumentFormat, EditOp, NodeTarget};

use crate::documents_service_fixture::{
    GroupSource, Harness, allow, api_author, create, deny, harness, harness_on, person, trace,
    unique,
};

fn commit_bytes(document: &DocumentId, base: &DocumentRevisionId, bytes: Vec<u8>) -> CommitRequest {
    CommitRequest {
        document_id: document.clone(),
        base_revision: Some(base.clone()),
        body: CommitBody::Bytes(bytes),
        author: api_author(),
        message: None,
        lock_holder_id: None,
        comments: CommentPolicy::default(),
        op_id: None,
    }
}

fn replace_title(
    document: &DocumentId,
    base: &DocumentRevisionId,
    node: &DocumentNodeId,
    expected_hash: String,
) -> CommitRequest {
    CommitRequest {
        body: CommitBody::Ops(vec![EditOp::ReplaceText {
            node: NodeTarget::Id(node.clone()),
            expected_hash: Some(expected_hash),
            text: "# Quarterly plan".to_owned(),
            content_as: ContentAs::Text,
        }]),
        ..commit_bytes(document, base, Vec::new())
    }
}

async fn import_markdown(
    h: &Harness,
    who: &AuthenticatedUser,
    text: &str,
) -> (DocumentId, DocumentRevisionId) {
    let (document, revision) = h
        .service
        .import(
            who,
            ImportRequest {
                folder: None,
                name: unique("notes"),
                format: Some(DocumentFormat::Markdown),
                bytes: text.as_bytes().to_vec(),
                author: api_author(),
            },
            &trace(),
        )
        .await
        .expect("import markdown");
    (document.id, revision.id)
}

#[tokio::test]
async fn create_folder_registers_an_authz_entity() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("owner");
    let request = NewFolderRequest {
        parent: None,
        name: unique("Drive"),
    };
    let folder = h
        .service
        .create_folder(&owner, request, &trace())
        .await
        .expect("create folder");
    let entity = h
        .access_entities
        .get_entity(EntityKind::Folder, folder.id.as_str())
        .await
        .expect("read entity")
        .expect("folder entity registered");
    assert!(entity.default_included);
    assert_eq!(entity.source, "code:files");
}

#[tokio::test]
async fn ops_commit_checks_anchors_then_moves_the_head() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("owner");
    let (document, first) = h
        .service
        .create_from_template(&owner, create("blank-markdown", None), &trace())
        .await
        .expect("create from template");
    assert!(document.name.ends_with(".md"));

    let (_, projection) = h
        .service
        .project(&owner, &document.id, None, &trace())
        .await
        .expect("project");
    let node = projection
        .nodes()
        .find(|node| node.attrs.get("path").map(String::as_str) == Some("line[1]"))
        .expect("title line");
    let stale = h
        .service
        .commit(
            &owner,
            replace_title(&document.id, &first.id, &node.id, "00000000".to_owned()),
            &trace(),
        )
        .await
        .expect_err("a wrong hash is refused");
    let message = stale.to_string();
    assert!(matches!(stale, DocumentsError::StaleAnchor { .. }));
    assert!(message.contains(node.id.as_str()), "{message}");
    assert!(message.contains("00000000"), "{message}");
    assert!(message.contains(&node.hash8), "{message}");
    assert!(message.contains("# Untitled"), "{message}");

    let outcome = h
        .service
        .commit(
            &owner,
            replace_title(&document.id, &first.id, &node.id, node.hash8.clone()),
            &trace(),
        )
        .await
        .expect("commit with the current hash");
    let CommitOutcome::Committed(landed) = outcome else {
        panic!("expected Committed, got {outcome:?}");
    };
    let LandedCommit {
        revision, warnings, ..
    } = *landed;
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(revision.parent_revision_id.as_ref(), Some(&first.id));
    let view = h
        .service
        .get(&owner, &document.id, &trace())
        .await
        .expect("get");
    assert_eq!(view.head.map(|r| r.id), Some(revision.id));
    assert_eq!(view.my_access, AccessLevel::Own);
    let history = h
        .service
        .history(
            &owner,
            &document.id,
            HistoryPage {
                limit: 10,
                before: None,
            },
            &trace(),
        )
        .await
        .expect("history");
    assert_eq!(history.len(), 2);
    let (_, text) = h
        .service
        .plain_text(&owner, &document.id, None, &trace())
        .await
        .expect("plain text");
    assert!(text.contains("Quarterly plan"), "{text}");
}

#[tokio::test]
async fn concurrent_docx_commits_conflict() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("owner");
    let (document, first) = h
        .service
        .create_from_template(&owner, create("blank-docx", None), &trace())
        .await
        .expect("create docx");
    let (_, bytes) = h
        .service
        .read_bytes(&owner, &document.id, None, &trace())
        .await
        .expect("read bytes");
    let winner = h
        .service
        .commit(
            &owner,
            commit_bytes(&document.id, &first.id, bytes.clone()),
            &trace(),
        )
        .await
        .expect("first commit");
    let CommitOutcome::Committed(landed) = winner else {
        panic!("expected Committed, got {winner:?}");
    };
    let head = landed.revision;
    let loser = h
        .service
        .commit(
            &owner,
            commit_bytes(&document.id, &first.id, bytes),
            &trace(),
        )
        .await
        .expect("second commit");
    let CommitOutcome::Conflict {
        head: reported,
        base,
    } = loser
    else {
        panic!("expected Conflict, got {loser:?}");
    };
    assert_eq!(reported.id, head.id);
    assert_eq!(base, Some(first.id));
}

#[tokio::test]
async fn concurrent_markdown_commits_merge_or_conflict() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("owner");
    let (document, first) = import_markdown(&h, &owner, "a\nb\nc\nd\ne\n").await;
    let ours = h
        .service
        .commit(
            &owner,
            commit_bytes(&document, &first, b"A\nb\nc\nd\ne\n".to_vec()),
            &trace(),
        )
        .await
        .expect("first commit");
    assert!(matches!(ours, CommitOutcome::Committed(_)), "{ours:?}");
    let theirs = h
        .service
        .commit(
            &owner,
            commit_bytes(&document, &first, b"a\nb\nc\nd\nE\n".to_vec()),
            &trace(),
        )
        .await
        .expect("second commit");
    let CommitOutcome::Merged(landed) = theirs else {
        panic!("expected Merged, got {theirs:?}");
    };
    let revision = landed.revision;
    let (_, merged) = h
        .service
        .read_bytes(&owner, &document, Some(&revision.id), &trace())
        .await
        .expect("read merged");
    assert_eq!(merged, b"A\nb\nc\nd\nE\n");

    let clash = h
        .service
        .commit(
            &owner,
            commit_bytes(&document, &first, b"X\nb\nc\nd\ne\n".to_vec()),
            &trace(),
        )
        .await
        .expect("third commit");
    assert!(matches!(clash, CommitOutcome::Conflict { .. }), "{clash:?}");
}

#[tokio::test]
async fn a_denying_hook_forbids_even_the_owner() {
    let open = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("owner");
    let (document, _) = open
        .service
        .create_from_template(&owner, create("blank-markdown", None), &trace())
        .await
        .expect("create");
    let closed = harness_on(deny(), GroupSource::new(&[]), open.storage.clone()).await;
    let refused = closed
        .service
        .get(&owner, &document.id, &trace())
        .await
        .expect_err("deny hook refuses");
    let DocumentsError::Forbidden { held, policy, .. } = refused else {
        panic!("expected Forbidden, got {refused:?}");
    };
    assert_eq!(held, Some(AccessLevel::Own));
    assert!(policy.is_some());
    let create_refused = closed
        .service
        .create_from_template(&owner, create("blank-markdown", None), &trace())
        .await
        .expect_err("deny hook refuses creation");
    assert!(matches!(create_refused, DocumentsError::Forbidden { .. }));
}

#[tokio::test]
async fn a_group_folder_grant_reaches_members_only() {
    let owner = person("owner");
    let sales = person("sales");
    let hr = person("hr");
    let source = GroupSource::new(&[(&sales.id, "sales"), (&hr.id, "hr")]);
    let h = harness(allow(), source).await;
    let folder = h
        .service
        .create_folder(
            &owner,
            NewFolderRequest {
                parent: None,
                name: unique("Sales"),
            },
            &trace(),
        )
        .await
        .expect("create folder");
    let (document, _) = h
        .service
        .create_from_template(&owner, create("blank-markdown", Some(&folder.id)), &trace())
        .await
        .expect("create");
    h.service
        .share(
            &owner,
            ShareRequest {
                target: GrantTarget::Folder(folder.id.clone()),
                subject_kind: "group".to_owned(),
                subject_value: "sales".to_owned(),
                subject_display: None,
                level: AccessLevel::View,
                valid_until: None,
                message: None,
            },
            &trace(),
        )
        .await
        .expect("share with sales");

    let seen = h
        .service
        .get(&sales, &document.id, &trace())
        .await
        .expect("sales member sees the document");
    assert_eq!(seen.my_access, AccessLevel::View);
    let listing = h
        .service
        .list_folder(
            &sales,
            ListFolderRequest {
                scope: ListScope::Folder,
                folder: Some(folder.id.clone()),
                query: None,
                sort: ListSort::Updated,
                cursor: None,
                limit: 50,
            },
            &trace(),
        )
        .await
        .expect("sales member lists the folder");
    assert!(
        listing
            .documents
            .iter()
            .any(|d| d.document.id == document.id)
    );

    let refused = h
        .service
        .get(&hr, &document.id, &trace())
        .await
        .expect_err("hr member is refused");
    assert!(
        matches!(refused, DocumentsError::NotFound { .. }),
        "{refused:?}"
    );
}

#[tokio::test]
async fn sharing_above_the_held_level_is_forbidden() {
    let owner = person("owner");
    let editor = person("editor");
    let h = harness(allow(), GroupSource::new(&[])).await;
    let (document, _) = h
        .service
        .create_from_template(&owner, create("blank-markdown", None), &trace())
        .await
        .expect("create");
    let share = |subject: &str, level| ShareRequest {
        target: GrantTarget::Document(document.id.clone()),
        subject_kind: "user".to_owned(),
        subject_value: subject.to_owned(),
        subject_display: None,
        level,
        valid_until: None,
        message: None,
    };
    h.service
        .share(
            &owner,
            share(editor.id.as_str(), AccessLevel::Share),
            &trace(),
        )
        .await
        .expect("owner shares");
    let refused = h
        .service
        .share(&editor, share("someone", AccessLevel::Own), &trace())
        .await
        .expect_err("own is never granted");
    assert!(
        matches!(refused, DocumentsError::OwnNotGrantable),
        "{refused:?}"
    );
    let unknown = h
        .service
        .share(
            &owner,
            ShareRequest {
                subject_kind: "department".to_owned(),
                ..share("x", AccessLevel::View)
            },
            &trace(),
        )
        .await
        .expect_err("unregistered subject kind");
    assert!(matches!(unknown, DocumentsError::UnknownSubjectKind { .. }));
}

#[tokio::test]
async fn a_lock_held_by_another_holder_blocks_commits() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("owner");
    let (document, first) = import_markdown(&h, &owner, "one\n").await;
    let outcome = h
        .service
        .acquire_lock(
            &owner,
            &document,
            AcquireLockRequest {
                holder_kind: LockHolderKind::Bridge,
                holder_id: "device-1".to_owned(),
                ttl: Duration::from_secs(60),
            },
            &trace(),
        )
        .await
        .expect("acquire");
    assert!(matches!(outcome, LockOutcome::Acquired(_)));
    let request = CommitRequest {
        lock_holder_id: Some("device-2".to_owned()),
        ..commit_bytes(&document, &first, b"two\n".to_vec())
    };
    let refused = h
        .service
        .commit(&owner, request, &trace())
        .await
        .expect_err("lock held elsewhere");
    let message = refused.to_string();
    assert!(matches!(refused, DocumentsError::LockHeld { .. }));
    assert!(message.contains("device-1"), "{message}");
    let holder = CommitRequest {
        lock_holder_id: Some("device-1".to_owned()),
        ..commit_bytes(&document, &first, b"two\n".to_vec())
    };
    let committed = h
        .service
        .commit(&owner, holder, &trace())
        .await
        .expect("the holder commits");
    assert!(matches!(committed, CommitOutcome::Committed(_)));
}
