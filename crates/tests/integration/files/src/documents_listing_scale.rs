//! The per-scope listing statements: grant visibility on a seeded set, an
//! index-only plan for the shared scope, batched child folder visibility and
//! the agent read retention window.

use std::sync::Arc;

use chrono::Utc;
use systemprompt_files::documents::agents::AgentCap;
use systemprompt_files::documents::jobs::DocumentAgentReadRetentionJob;
use systemprompt_files::documents::models::{
    GrantTarget, ListScope, ListSort, ListedDocument, NewFolder, NewGrant,
};
use systemprompt_files::documents::repository::{
    AgentReadRecord, ROOT_LISTING_SQL, SHARED_LISTING_SQL,
};
use systemprompt_files::documents::service::{ListFolderRequest, NewFolderRequest, ShareRequest};
use systemprompt_governance::authz::{
    AuthzDecision, AuthzDecisionHook, AuthzRequest, DenyReason, EntityRef, SharedAuthzHook,
};
use systemprompt_identifiers::{DocumentFolderId, DocumentId, TraceId, UserId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AccessLevel, EditOp};
use systemprompt_test_fixtures::test_db_pool;

use crate::documents_service_fixture::{
    GroupSource, Harness, allow, create, harness, person, trace, unique,
};

const SHARED_SOURCE: &str =
    include_str!("../../../../domain/files/src/documents/repository/listing_scopes_shared.rs");
const ROOT_SOURCE: &str =
    include_str!("../../../../domain/files/src/documents/repository/listing_scopes_root.rs");

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

async fn folder(
    h: &Harness,
    owner: &AuthenticatedUser,
    parent: Option<&DocumentFolderId>,
    name: &str,
) -> DocumentFolderId {
    h.service
        .create_folder(
            owner,
            NewFolderRequest {
                parent: parent.cloned(),
                name: name.to_owned(),
            },
            &trace(),
        )
        .await
        .expect("folder")
        .id
}

async fn share(h: &Harness, owner: &AuthenticatedUser, target: GrantTarget, reader: &UserId) {
    h.service
        .share(
            owner,
            ShareRequest {
                target,
                subject_kind: "user".to_owned(),
                subject_value: reader.as_str().to_owned(),
                subject_display: None,
                level: AccessLevel::View,
                valid_until: None,
                message: None,
                via_agent: None,
            },
            &trace(),
        )
        .await
        .expect("share");
}

async fn stored_grant(
    h: &Harness,
    document: &DocumentId,
    reader: &UserId,
    owner: &UserId,
    level: AccessLevel,
    expired: bool,
) {
    let mut tx = h.repos.documents.begin().await.expect("begin");
    h.repos
        .grants
        .insert(
            &mut tx,
            NewGrant {
                target: GrantTarget::Document(document.clone()),
                subject_kind: "user".to_owned(),
                subject_value: reader.as_str().to_owned(),
                subject_display: None,
                access_level: level,
                granted_by: owner.clone(),
                valid_until: expired.then(|| Utc::now() - chrono::Duration::hours(1)),
                trace_id: TraceId::generate(),
                reason: None,
                via_agent: None,
            },
        )
        .await
        .expect("insert grant");
    tx.commit().await.expect("commit");
}

fn request(scope: ListScope, folder: Option<DocumentFolderId>) -> ListFolderRequest {
    ListFolderRequest {
        scope,
        folder,
        query: None,
        sort: ListSort::Updated,
        cursor: None,
        limit: 100,
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

fn access_of(listed: &[ListedDocument], id: &DocumentId) -> Option<AccessLevel> {
    listed
        .iter()
        .find(|l| l.document.id == *id)
        .map(|l| l.my_access)
}

#[tokio::test]
async fn every_scope_keeps_grant_visibility_on_a_seeded_set() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("scope-owner");
    let reader = person("scope-reader");
    let team = folder(&h, &owner, None, &unique("Team")).await;
    let nested = folder(&h, &owner, Some(&team), &unique("Nested")).await;
    let direct = named(&h, &owner, &unique("direct"), None).await;
    let through_folder = named(&h, &owner, &unique("nested-doc"), Some(&nested)).await;
    let trashed = named(&h, &owner, &unique("trashed"), None).await;
    let expired = named(&h, &owner, &unique("expired"), None).await;
    let stored_own = named(&h, &owner, &unique("stored-own"), None).await;
    let private = named(&h, &owner, &unique("private"), None).await;
    let own = named(&h, &reader, &unique("reader-own"), None).await;
    share(
        &h,
        &owner,
        GrantTarget::Document(direct.clone()),
        &reader.id,
    )
    .await;
    share(&h, &owner, GrantTarget::Folder(team.clone()), &reader.id).await;
    share(
        &h,
        &owner,
        GrantTarget::Document(trashed.clone()),
        &reader.id,
    )
    .await;
    h.service
        .trash(&owner, &trashed, &trace())
        .await
        .expect("trash");
    stored_grant(&h, &expired, &reader.id, &owner.id, AccessLevel::Edit, true).await;
    stored_grant(
        &h,
        &stored_own,
        &reader.id,
        &owner.id,
        AccessLevel::Own,
        false,
    )
    .await;

    let shared = list(&h, &reader, request(ListScope::Shared, None)).await;
    assert_eq!(access_of(&shared, &direct), Some(AccessLevel::View));
    assert_eq!(
        access_of(&shared, &through_folder),
        Some(AccessLevel::View),
        "a grant on an ancestor folder reaches the document"
    );
    assert_eq!(
        access_of(&shared, &stored_own),
        Some(AccessLevel::Share),
        "a stored own grant reports as share"
    );
    for hidden in [&trashed, &expired, &private, &own] {
        assert_eq!(access_of(&shared, hidden), None, "{hidden:?}");
    }

    let mine = list(&h, &reader, request(ListScope::Mine, None)).await;
    assert_eq!(access_of(&mine, &own), Some(AccessLevel::Own));
    assert_eq!(access_of(&mine, &direct), None);

    let in_nested = list(&h, &reader, request(ListScope::Folder, Some(nested))).await;
    assert_eq!(
        access_of(&in_nested, &through_folder),
        Some(AccessLevel::View)
    );

    let at_root = list(&h, &reader, request(ListScope::Folder, None)).await;
    assert_eq!(access_of(&at_root, &direct), Some(AccessLevel::View));
    assert_eq!(access_of(&at_root, &own), Some(AccessLevel::Own));
    for hidden in [&trashed, &expired, &private] {
        assert_eq!(access_of(&at_root, hidden), None, "{hidden:?}");
    }

    let recent = list(&h, &reader, request(ListScope::Recent, None)).await;
    assert_eq!(
        access_of(&recent, &direct),
        None,
        "a document the reader never touched is not recent"
    );
}

fn seq_scans_on(plan: &serde_json::Value, relation: &str, found: &mut Vec<String>) {
    match plan {
        serde_json::Value::Object(node) => {
            if node.get("Node Type").and_then(serde_json::Value::as_str) == Some("Seq Scan")
                && node
                    .get("Relation Name")
                    .and_then(serde_json::Value::as_str)
                    == Some(relation)
            {
                found.push(plan.to_string());
            }
            for child in node.values() {
                seq_scans_on(child, relation, found);
            }
        },
        serde_json::Value::Array(items) => {
            for item in items {
                seq_scans_on(item, relation, found);
            }
        },
        _ => {},
    }
}

fn assert_is_the_production_statement(source: &str, statement: &str) {
    let literal = format!("r#\"{statement}\"#");
    assert_eq!(
        source.matches(&literal).count(),
        2,
        "the exposed statement is the one the query! macro runs"
    );
}

async fn documents_seq_scans(statement: &str, reader: &UserId) -> Vec<String> {
    let pool = test_db_pool().await.write_pool();
    let mut tx = pool.begin().await.expect("begin");
    sqlx::query("SET LOCAL enable_seqscan = off")
        .execute(&mut *tx)
        .await
        .expect("disable seq scans");
    let explain = format!("EXPLAIN (FORMAT JSON) {statement}");
    let plan = sqlx::query_scalar::<_, serde_json::Value>(sqlx::AssertSqlSafe(explain))
        .bind(reader.as_str())
        .bind(None::<String>)
        .bind("updated")
        .bind(None::<chrono::DateTime<Utc>>)
        .bind(None::<String>)
        .bind(None::<uuid::Uuid>)
        .bind(51_i64)
        .bind(vec!["user".to_owned()])
        .bind(vec![reader.as_str().to_owned()])
        .fetch_one(&mut *tx)
        .await
        .expect("explain");
    tx.rollback().await.expect("rollback");
    let mut found = Vec::new();
    seq_scans_on(&plan, "documents", &mut found);
    found
}

async fn seeded_reader(h: &Harness, prefix: &str) -> AuthenticatedUser {
    let owner = person(&format!("{prefix}-owner"));
    let reader = person(&format!("{prefix}-reader"));
    let team = folder(h, &owner, None, &unique(prefix)).await;
    for n in 0..20 {
        let at = (n % 2 == 0).then_some(&team);
        let id = named(h, &owner, &unique(&format!("{prefix}-{n}")), at).await;
        if n % 4 == 0 {
            share(h, &owner, GrantTarget::Document(id), &reader.id).await;
        }
    }
    named(h, &reader, &unique(&format!("{prefix}-own")), None).await;
    share(h, &owner, GrantTarget::Folder(team), &reader.id).await;
    reader
}

#[tokio::test]
async fn the_shared_scope_reaches_documents_through_indexes() {
    assert_is_the_production_statement(SHARED_SOURCE, SHARED_LISTING_SQL);
    let h = harness(allow(), GroupSource::new(&[])).await;
    let reader = seeded_reader(&h, "plan-shared").await;
    let found = documents_seq_scans(SHARED_LISTING_SQL, &reader.id).await;
    assert!(found.is_empty(), "shared scope scans documents: {found:?}");
}

#[tokio::test]
async fn the_root_scope_reaches_documents_through_indexes() {
    assert_is_the_production_statement(ROOT_SOURCE, ROOT_LISTING_SQL);
    let h = harness(allow(), GroupSource::new(&[])).await;
    let reader = seeded_reader(&h, "plan-root").await;
    let found = documents_seq_scans(ROOT_LISTING_SQL, &reader.id).await;
    assert!(found.is_empty(), "root scope scans documents: {found:?}");
}

#[tokio::test]
async fn the_statement_check_rejects_a_statement_the_macro_does_not_run() {
    let altered = SHARED_LISTING_SQL.replacen("LIMIT $7", "LIMIT $7 + 1", 1);
    let literal = format!("r#\"{altered}\"#");
    assert_eq!(SHARED_SOURCE.matches(&literal).count(), 0);
}

#[tokio::test]
async fn fifty_subfolders_list_in_one_visible_page() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("children-owner");
    let reader = person("children-reader");
    let parent = folder(&h, &owner, None, &unique("Parent")).await;
    let mut expected = Vec::new();
    for n in 0..50 {
        expected.push(folder(&h, &owner, Some(&parent), &format!("child-{n:02}")).await);
    }
    let unshared = folder(&h, &owner, None, &unique("Unshared")).await;
    let lone = folder(&h, &owner, Some(&unshared), "lone").await;
    share(&h, &owner, GrantTarget::Folder(parent.clone()), &reader.id).await;

    let listed = h
        .service
        .list_folder(
            &reader,
            request(ListScope::Folder, Some(parent.clone())),
            &trace(),
        )
        .await
        .expect("list children");
    let seen: Vec<DocumentFolderId> = listed.folders.iter().map(|f| f.id.clone()).collect();
    assert_eq!(seen, expected, "every child, ordered by name");

    let mut filtered = request(ListScope::Folder, Some(parent));
    filtered.query = Some("child-4".to_owned());
    let narrowed = h
        .service
        .list_folder(&reader, filtered, &trace())
        .await
        .expect("filtered children");
    assert_eq!(narrowed.folders.len(), 10);

    let at_root = h
        .service
        .list_folder(&reader, request(ListScope::Folder, None), &trace())
        .await
        .expect("root");
    let root_ids: Vec<DocumentFolderId> = at_root.folders.iter().map(|f| f.id.clone()).collect();
    assert!(!root_ids.contains(&unshared));
    assert!(!root_ids.contains(&lone));
}

#[derive(Debug)]
struct ChildDenyHook {
    open: Vec<DocumentFolderId>,
}

#[async_trait::async_trait]
impl AuthzDecisionHook for ChildDenyHook {
    async fn evaluate(&self, req: AuthzRequest) -> AuthzDecision {
        self.probe(req).await
    }

    async fn probe(&self, req: AuthzRequest) -> AuthzDecision {
        match &req.entity {
            EntityRef::Folder(id) if !self.open.contains(id) => AuthzDecision::Deny {
                reason: DenyReason::HookUnavailable {
                    policy: "child-deny".to_owned(),
                    detail: "closed child".to_owned(),
                },
                policy: "child-deny".to_owned(),
            },
            _ => AuthzDecision::Allow,
        }
    }
}

#[tokio::test]
async fn a_folder_whose_first_pages_of_children_are_all_denied_still_lists() {
    let probe = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("denied-children");
    let parent = probe
        .repos
        .folders
        .insert(NewFolder {
            parent_id: None,
            name: unique("Denied"),
            owner_user_id: owner.id.clone(),
            created_by: owner.id.clone(),
        })
        .await
        .expect("parent")
        .id;
    for n in 0..450 {
        probe
            .repos
            .folders
            .insert(NewFolder {
                parent_id: Some(parent.clone()),
                name: format!("a-{n:03}"),
                owner_user_id: owner.id.clone(),
                created_by: owner.id.clone(),
            })
            .await
            .expect("denied child");
    }
    let open_child = probe
        .repos
        .folders
        .insert(NewFolder {
            parent_id: Some(parent.clone()),
            name: "z-open".to_owned(),
            owner_user_id: owner.id.clone(),
            created_by: owner.id.clone(),
        })
        .await
        .expect("open child")
        .id;
    let hook: SharedAuthzHook = Arc::new(ChildDenyHook {
        open: vec![parent.clone(), open_child.clone()],
    });
    let h = harness(hook, GroupSource::new(&[])).await;

    let listed = tokio::time::timeout(
        std::time::Duration::from_secs(120),
        h.service.list_folder(
            &owner,
            request(ListScope::Folder, Some(parent.clone())),
            &trace(),
        ),
    )
    .await
    .expect("the listing terminates")
    .expect("list");
    let seen: Vec<DocumentFolderId> = listed.folders.iter().map(|f| f.id.clone()).collect();
    assert_eq!(
        seen,
        vec![open_child],
        "denied children are skipped, not re-fetched"
    );
}

#[tokio::test]
async fn agent_read_retention_deletes_only_rows_past_the_window() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("retention-owner");
    let document = named(&h, &owner, &unique("retained"), None).await;
    let db = test_db_pool().await;
    let pool = db.write_pool();
    for days in [400_i32, 91, 10, 0] {
        sqlx::query(
            "INSERT INTO document_agent_reads \
             (document_id, user_id, agent_name, attended, action, effective_cap, cap_source, \
              created_at) \
             VALUES ($1::text::uuid, $2, 'retention-agent', false, 'view', 'read', 'policy', \
                     now() - make_interval(days => $3))",
        )
        .bind(document.as_str())
        .bind(owner.id.as_str())
        .bind(days)
        .execute(&*pool)
        .await
        .expect("seed agent read");
    }
    DocumentAgentReadRetentionJob::purge(&db, 90)
        .await
        .expect("purge");
    let left: Vec<i32> = sqlx::query_scalar(
        "SELECT EXTRACT(DAY FROM now() - created_at)::int FROM document_agent_reads \
         WHERE document_id = $1::text::uuid ORDER BY created_at",
    )
    .bind(document.as_str())
    .fetch_all(&*pool)
    .await
    .expect("remaining");
    assert_eq!(left, vec![10, 0]);
}

#[tokio::test]
async fn an_agent_read_is_recorded_once_per_day() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("dedup-owner");
    let document = named(&h, &owner, &unique("dedup"), None).await;
    let trace_id = TraceId::generate();
    let record = || AgentReadRecord {
        document_id: &document,
        user_id: &owner.id,
        agent: "dedup-agent",
        attended: true,
        action: "view",
        cap: AgentCap::Read,
        cap_source: "policy",
        trace_id: &trace_id,
    };
    for _ in 0..3 {
        h.repos
            .documents
            .insert_agent_read(record())
            .await
            .expect("record");
    }
    h.repos
        .documents
        .insert_agent_read(AgentReadRecord {
            action: "list",
            ..record()
        })
        .await
        .expect("record list");
    let rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM document_agent_reads WHERE document_id = $1::text::uuid",
    )
    .bind(document.as_str())
    .fetch_one(&*test_db_pool().await.write_pool())
    .await
    .expect("count");
    assert_eq!(rows, 2, "one row per action per day");
    let (reads, first_trace, repeated): (i32, Option<String>, bool) = sqlx::query_as(
        "SELECT reads, trace_id, last_read_at IS NOT NULL FROM document_agent_reads \
         WHERE document_id = $1::text::uuid AND action = 'view'",
    )
    .bind(document.as_str())
    .fetch_one(&*test_db_pool().await.write_pool())
    .await
    .expect("view row");
    assert_eq!(reads, 3, "repeat reads are counted on the day's row");
    assert_eq!(first_trace.as_deref(), Some(trace_id.as_str()));
    assert!(repeated);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_first_agent_reads_land_on_one_row_with_the_latest_trace() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("dedup-race");
    let document = named(&h, &owner, &unique("dedup-race"), None).await;
    let traces: Vec<TraceId> = (0..8).map(|_| TraceId::generate()).collect();
    let reads = traces.iter().map(|trace_id| {
        h.repos.documents.insert_agent_read(AgentReadRecord {
            document_id: &document,
            user_id: &owner.id,
            agent: "race-agent",
            attended: true,
            action: "view",
            cap: AgentCap::Read,
            cap_source: "policy",
            trace_id,
        })
    });
    for outcome in futures::future::join_all(reads).await {
        outcome.expect("record");
    }
    let last = TraceId::generate();
    h.repos
        .documents
        .insert_agent_read(AgentReadRecord {
            document_id: &document,
            user_id: &owner.id,
            agent: "race-agent",
            attended: true,
            action: "view",
            cap: AgentCap::Read,
            cap_source: "policy",
            trace_id: &last,
        })
        .await
        .expect("record last");
    let (rows, reads, last_trace): (i64, i64, Option<String>) = sqlx::query_as(
        "SELECT COUNT(*), SUM(reads)::bigint, MAX(last_trace_id) FROM document_agent_reads \
         WHERE document_id = $1::text::uuid",
    )
    .bind(document.as_str())
    .fetch_one(&*test_db_pool().await.write_pool())
    .await
    .expect("rows");
    assert_eq!(rows, 1, "racing first reads share the day's row");
    assert_eq!(reads, 9);
    assert_eq!(last_trace.as_deref(), Some(last.as_str()));
}
