//! DB-backed tests for `AgentRunRepository`: claim exclusivity, lease
//! renewal, stale reclaim and event batches. Each test uses its own
//! `origin_kind` so concurrent tests never see each other's rows. Tests fail
//! when `DATABASE_URL` is unset.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use systemprompt_agent_runs::repository::NewAgentRun;
use systemprompt_agent_runs::{
    AgentRunRepository, PermissionMode, RunEvent, RunEventPayload, RunRequest,
};
use systemprompt_identifiers::{AgentRunId, HostSessionId, UserId};
use systemprompt_test_fixtures::test_db_pool;

pub(crate) struct Fixture {
    pub repo: AgentRunRepository,
    pub origin_kind: String,
    pub user_id: UserId,
}

pub(crate) async fn fixture() -> Fixture {
    let pool = test_db_pool().await;
    let repo = AgentRunRepository::new(&pool);
    let tag = uuid::Uuid::new_v4().simple().to_string();
    let user_id = UserId::new(format!("headless-test-{tag}"));
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $1 || '@test.invalid')")
        .bind(user_id.as_str())
        .execute(repo.write_pool())
        .await
        .expect("insert test user");
    Fixture {
        repo,
        origin_kind: format!("test-{tag}"),
        user_id,
    }
}

pub(crate) fn request() -> RunRequest {
    RunRequest {
        host: "claude-code".parse().expect("host"),
        prompt: "hello".into(),
        system_prompt_append: None,
        persona: None,
        model: None,
        allowed_tools: Vec::new(),
        disallowed_tools: Vec::new(),
        permission_mode: PermissionMode::Default,
        permission_prompts_none: false,
        workspace: PathBuf::from("/tmp/ws"),
        add_dirs: Vec::new(),
        mcp_servers: Default::default(),
        output_schema: None,
        max_turns: None,
        max_budget_usd: None,
        wall_clock: Duration::from_secs(60),
        resume_session_id: None,
        attempt: 1,
        attribution_scopes: Vec::new(),
        plugin_id: None,
        plugin_dirs: Vec::new(),
    }
}

pub(crate) async fn queue(f: &Fixture) -> AgentRunId {
    let id = AgentRunId::generate();
    f.repo
        .insert_run(NewAgentRun {
            id: &id,
            origin_kind: &f.origin_kind,
            origin_id: "origin-1",
            user_id: &f.user_id,
            request: &request(),
            not_before: None,
        })
        .await
        .expect("insert run");
    id
}

async fn expire_lease(f: &Fixture, id: &AgentRunId) {
    sqlx::query("UPDATE agent_runs SET lease_until = NOW() - INTERVAL '1 second' WHERE id = $1")
        .bind(id.as_str())
        .execute(f.repo.write_pool())
        .await
        .expect("expire lease");
}

#[tokio::test]
async fn concurrent_claimers_never_share_a_run() {
    let f = Arc::new(fixture().await);
    for _ in 0..6 {
        queue(&f).await;
    }
    let mut handles = Vec::new();
    for n in 0..4 {
        let f = Arc::clone(&f);
        handles.push(tokio::spawn(async move {
            let mut got = Vec::new();
            while let Some(row) = f
                .repo
                .claim_queued(
                    &f.origin_kind,
                    &format!("claimer-{n}"),
                    Duration::from_secs(90),
                )
                .await
                .expect("claim")
            {
                assert_eq!(row.status, "preparing");
                got.push(row.id);
            }
            got
        }));
    }
    let mut all = Vec::new();
    for h in handles {
        all.extend(h.await.expect("join"));
    }
    let total = all.len();
    all.sort();
    all.dedup();
    assert_eq!(total, 6);
    assert_eq!(all.len(), 6);
}

#[tokio::test]
async fn renew_fails_after_another_claimer_took_the_run() {
    let f = fixture().await;
    let id = queue(&f).await;
    let lease = Duration::from_secs(90);
    f.repo
        .claim_queued(&f.origin_kind, "a", lease)
        .await
        .expect("claim")
        .expect("row");
    assert!(f.repo.renew_lease(&id, "a", lease).await.expect("renew"));
    expire_lease(&f, &id).await;
    f.repo
        .reclaim_stale(&f.origin_kind, 3)
        .await
        .expect("reclaim");
    f.repo
        .claim_queued(&f.origin_kind, "b", lease)
        .await
        .expect("claim")
        .expect("row");
    assert!(!f.repo.renew_lease(&id, "a", lease).await.expect("renew"));
    assert!(f.repo.renew_lease(&id, "b", lease).await.expect("renew"));
}

#[tokio::test]
async fn stale_reclaim_requeues_with_resume_then_fails() {
    let f = fixture().await;
    let id = queue(&f).await;
    let lease = Duration::from_secs(90);
    f.repo
        .claim_queued(&f.origin_kind, "a", lease)
        .await
        .expect("claim")
        .expect("row");
    let session = HostSessionId::try_new("host-session-1").expect("session id");
    f.repo
        .update_host_session(&id, &session)
        .await
        .expect("session");
    expire_lease(&f, &id).await;
    assert_eq!(
        f.repo
            .reclaim_stale(&f.origin_kind, 2)
            .await
            .expect("reclaim"),
        vec![id.clone()]
    );

    let row = f.repo.find_run(&id).await.expect("find").expect("row");
    assert_eq!(row.status, "queued");
    assert_eq!(row.attempt, 2);
    assert!(row.claimed_by.is_none());
    let req: RunRequest = serde_json::from_value(row.request).expect("request");
    assert_eq!(req.resume_session_id, Some(session));
    assert_eq!(req.attempt, 2);

    f.repo
        .claim_queued(&f.origin_kind, "b", lease)
        .await
        .expect("claim")
        .expect("row");
    expire_lease(&f, &id).await;
    f.repo
        .reclaim_stale(&f.origin_kind, 2)
        .await
        .expect("reclaim");
    let row = f.repo.find_run(&id).await.expect("find").expect("row");
    assert_eq!(row.status, "failed");
    assert_eq!(row.error_class.as_deref(), Some("host_crashed"));
    assert!(row.finished_at.is_some());
}

fn event(seq: i64, payload: RunEventPayload) -> RunEvent {
    RunEvent {
        seq,
        kind: payload.kind(),
        tool_name: None,
        tool_use_id: payload.tool_use_id().map(str::to_owned),
        payload,
        created_at: Utc::now(),
    }
}

#[tokio::test]
async fn events_batch_round_trips_in_seq_order() {
    let f = fixture().await;
    let id = queue(&f).await;
    let batch: Vec<RunEvent> = (1..=5)
        .rev()
        .map(|n| {
            event(
                n,
                RunEventPayload::AssistantText {
                    text: format!("t{n}"),
                },
            )
        })
        .chain(std::iter::once(event(
            6,
            RunEventPayload::ToolResult {
                tool_use_id: "tu-1".into(),
                content: "ok".into(),
                is_error: false,
            },
        )))
        .collect();
    f.repo
        .insert_events_batch(&id, &batch)
        .await
        .expect("insert");
    f.repo
        .insert_events_batch(&id, &batch[..1])
        .await
        .expect("replay");
    assert_eq!(f.repo.count_events(&id).await.expect("count"), 6);

    let after = f.repo.list_events_after(&id, 2, 10).await.expect("list");
    let seqs: Vec<i64> = after.iter().map(|e| e.seq).collect();
    assert_eq!(seqs, vec![3, 4, 5, 6]);
    assert_eq!(after[3].tool_use_id.as_deref(), Some("tu-1"));
    assert_eq!(after[3].tool_name, None);
    assert_eq!(
        after[0].payload,
        RunEventPayload::AssistantText { text: "t3".into() }
    );
    assert_eq!(
        f.repo
            .list_events_after(&id, 0, 2)
            .await
            .expect("list")
            .len(),
        2
    );
}
