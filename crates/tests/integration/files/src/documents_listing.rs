use systemprompt_files::documents::model::Media;
use systemprompt_files::documents::models::{
    CommentAnchor, GrantTarget, ListScope, ListSort, ListedDocument,
};
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{
    CommentPolicy, CommentRequest, CommitRequest, ListFolderRequest, NewFolderRequest, ShareRequest,
};
use systemprompt_identifiers::{DocumentFolderId, DocumentId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AccessLevel, EditOp};

use crate::documents_service_fixture::{
    GroupSource, Harness, allow, api_author, create, harness, person, trace, unique,
};

async fn named(
    h: &Harness,
    owner: &AuthenticatedUser,
    name: &str,
    folder: Option<&DocumentFolderId>,
) -> DocumentId {
    let mut request = create("blank-text", folder);
    request.name = name.to_owned();
    request.ops = vec![EditOp::SetText {
        text: "hello\n".to_owned(),
    }];
    h.service
        .create_from_template(owner, request, &trace())
        .await
        .expect("create")
        .0
        .id
}

async fn share(
    h: &Harness,
    owner: &AuthenticatedUser,
    target: GrantTarget,
    subject: (&str, &str),
    level: AccessLevel,
) {
    h.service
        .share(
            owner,
            ShareRequest {
                target,
                subject_kind: subject.0.to_owned(),
                subject_value: subject.1.to_owned(),
                subject_display: None,
                level,
                valid_until: None,
                message: None,
                via_agent: None,
            },
            &trace(),
        )
        .await
        .expect("share");
}

fn listing(scope: ListScope, query: Option<&str>, sort: ListSort) -> ListFolderRequest {
    ListFolderRequest {
        scope,
        folder: None,
        query: query.map(str::to_owned),
        sort,
        cursor: None,
        limit: 50,
    }
}

async fn list(
    h: &Harness,
    who: &AuthenticatedUser,
    request: ListFolderRequest,
) -> Vec<ListedDocument> {
    h.service
        .list_folder(who, request, &trace())
        .await
        .expect("list")
        .documents
}

fn ids(listed: &[ListedDocument]) -> Vec<DocumentId> {
    listed.iter().map(|l| l.document.id.clone()).collect()
}

#[tokio::test]
async fn shared_lists_folder_and_group_grants_with_size_and_access() {
    let reader = person("shared-reader");
    let h = harness(allow(), GroupSource::new(&[(&reader.id, "sales")])).await;
    let owner = person("shared-owner");
    let folder = h
        .service
        .create_folder(
            &owner,
            NewFolderRequest {
                parent: None,
                name: unique("Team"),
            },
            &trace(),
        )
        .await
        .expect("folder")
        .id;
    let in_folder = named(&h, &owner, &unique("folder-doc"), Some(&folder)).await;
    let by_group = named(&h, &owner, &unique("group-doc"), None).await;
    let unshared = named(&h, &owner, &unique("private-doc"), None).await;
    let own = named(&h, &reader, &unique("own-doc"), None).await;
    share(
        &h,
        &owner,
        GrantTarget::Folder(folder),
        ("user", reader.id.as_str()),
        AccessLevel::View,
    )
    .await;
    share(
        &h,
        &owner,
        GrantTarget::Document(by_group.clone()),
        ("group", "sales"),
        AccessLevel::Edit,
    )
    .await;

    let shared = list(
        &h,
        &reader,
        listing(ListScope::Shared, None, ListSort::Updated),
    )
    .await;
    let seen = ids(&shared);
    assert!(seen.contains(&in_folder), "a folder grant is shared");
    assert!(seen.contains(&by_group), "a group grant is shared");
    assert!(!seen.contains(&unshared));
    assert!(
        !seen.contains(&own),
        "shared excludes the caller's own documents"
    );
    let group_row = shared
        .iter()
        .find(|l| l.document.id == by_group)
        .expect("group row");
    assert_eq!(group_row.my_access, AccessLevel::Edit);
    assert_eq!(group_row.size_bytes, Some(6));

    let mine = list(
        &h,
        &reader,
        listing(ListScope::Mine, None, ListSort::Updated),
    )
    .await;
    let mine_row = mine.iter().find(|l| l.document.id == own).expect("own row");
    assert_eq!(mine_row.my_access, AccessLevel::Own);
}

#[tokio::test]
async fn recent_orders_by_the_callers_last_touch() {
    let reader = person("recent-reader");
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("recent-owner");
    let commented = named(&h, &owner, &unique("commented"), None).await;
    let edited = named(&h, &owner, &unique("edited"), None).await;
    let untouched = named(&h, &owner, &unique("untouched"), None).await;
    for id in [&commented, &edited, &untouched] {
        share(
            &h,
            &owner,
            GrantTarget::Document(id.clone()),
            ("user", reader.id.as_str()),
            AccessLevel::Edit,
        )
        .await;
    }
    let (_, projection) = h
        .service
        .project(&reader, &commented, None, &trace())
        .await
        .expect("project");
    let node = projection
        .nodes()
        .find(|n| n.text.as_deref() == Some("hello"))
        .expect("line")
        .id
        .clone();
    h.service
        .create_comment(
            &reader,
            CommentRequest {
                range: None,
                mentions: Vec::new(),
                document: commented.clone(),
                anchor: Some(CommentAnchor {
                    node,
                    find: None,
                    nth: None,
                }),
                body: "seen".to_owned(),
                parent_id: None,
                mention_options: vec![],
            },
            &trace(),
        )
        .await
        .expect("comment");
    let base = h
        .service
        .get(&reader, &edited, &trace())
        .await
        .expect("get")
        .head
        .map(|head| head.id);
    h.service
        .commit(
            &reader,
            CommitRequest {
                document_id: edited.clone(),
                base_revision: base,
                body: CommitBody::Ops(vec![EditOp::SetText {
                    text: "changed\n".to_owned(),
                }]),
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
        .expect("edit");
    let recent = ids(&list(
        &h,
        &reader,
        listing(ListScope::Recent, None, ListSort::Recent),
    )
    .await);
    assert_eq!(recent, vec![edited, commented], "{recent:?}");
    assert!(!recent.contains(&untouched));
}

#[tokio::test]
async fn the_name_query_escapes_like_wildcards_and_pages_by_name() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("escape-owner");
    let prefix = unique("q");
    let literal = named(&h, &owner, &format!("{prefix}-50%_off"), None).await;
    let lookalike = named(&h, &owner, &format!("{prefix}-50xyoff"), None).await;
    let found = ids(&list(
        &h,
        &owner,
        listing(
            ListScope::Mine,
            Some(&format!("{prefix}-50%_")),
            ListSort::Name,
        ),
    )
    .await);
    assert_eq!(found, vec![literal.clone()], "% and _ match literally");

    let third = named(&h, &owner, &format!("{prefix}-zz"), None).await;
    let mut request = listing(ListScope::Mine, Some(&prefix), ListSort::Name);
    request.limit = 2;
    let first = h
        .service
        .list_folder(&owner, request.clone(), &trace())
        .await
        .expect("first page");
    assert_eq!(ids(&first.documents), vec![literal, lookalike]);
    request.cursor = first.next_cursor;
    let second = h
        .service
        .list_folder(&owner, request, &trace())
        .await
        .expect("second page");
    assert_eq!(ids(&second.documents), vec![third]);
    assert!(second.next_cursor.is_none());
}

#[tokio::test]
async fn a_root_listing_skips_folders_the_caller_cannot_see() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let other = person("root-other");
    let caller = person("root-caller");
    let hidden = h
        .service
        .create_folder(
            &other,
            NewFolderRequest {
                parent: None,
                name: unique("Hidden"),
            },
            &trace(),
        )
        .await
        .expect("folder")
        .id;
    let own = h
        .service
        .create_folder(
            &caller,
            NewFolderRequest {
                parent: None,
                name: unique("Own"),
            },
            &trace(),
        )
        .await
        .expect("folder")
        .id;
    let listed = h
        .service
        .list_folder(
            &caller,
            listing(ListScope::Folder, None, ListSort::Name),
            &trace(),
        )
        .await
        .expect("a root listing ignores folders the caller cannot see");
    let folders: Vec<DocumentFolderId> = listed.folders.iter().map(|f| f.id.clone()).collect();
    assert!(folders.contains(&own), "{folders:?}");
    assert!(!folders.contains(&hidden), "{folders:?}");
}
