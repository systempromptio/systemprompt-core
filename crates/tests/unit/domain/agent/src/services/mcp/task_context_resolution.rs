//! How an MCP tool call acquires the task and context it runs under.
//!
//! `ensure_task_exists` is the entry point every MCP tool call passes through.
//! It decides three things that matter: whether a parent task is inherited
//! rather than duplicated, whether a session's context is reused across calls,
//! and — the security-relevant one — what happens when the caller supplies a
//! context id belonging to somebody else.

use systemprompt_agent::services::mcp::task_helper::ensure_task_exists;
use systemprompt_identifiers::{
    Actor, AgentName, ContextId, SessionId, TaskId, TraceId, UserId,
};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_test_fixtures::ensure_test_bootstrap;

use crate::repository::{repos, seed_user_and_session, try_pool};

fn context_for(user: &UserId, session: &SessionId, context_id: ContextId) -> RequestContext {
    let mut rc = RequestContext::new(
        session.clone(),
        TraceId::generate(),
        context_id,
        AgentName::new("mcp-caller"),
    );
    rc.auth.actor = Actor::user(user.clone());
    rc
}

// `ContextId::new_unchecked("")` is the shape a caller arrives with when it has
// no context yet; the resolver keys its first branch on the id being empty.
fn empty_context() -> ContextId {
    ContextId::new_unchecked("")
}

// Why: a nested tool call inherits its parent's task. Minting a second task
// would split one logical operation into two rows and make `is_owner` a lie —
// the nested caller would go on to close a task it does not own.
#[tokio::test]
async fn a_call_that_already_carries_a_task_reuses_it_and_disclaims_ownership() {
    let Some(pool) = try_pool().await else {
        return;
    };
    ensure_test_bootstrap();
    let repositories = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let parent_task = TaskId::generate();

    let mut rc = context_for(&user, &session, ContextId::generate());
    rc.execution.task_id = Some(parent_task.clone());

    let result = ensure_task_exists(&repositories, &mut rc, "tool", "srv")
        .await
        .expect("an inherited task is not an error");

    assert_eq!(result.task_id, parent_task, "the parent task must be reused");
    assert!(
        !result.is_owner,
        "an inherited task is not owned by this call, so it must not be closed by it"
    );
}

// Why: a caller with no context gets one, and the id it is given must be
// written back into the request context — everything downstream reads it from
// there, so returning it without storing it would leave the task attached to
// an empty context.
#[tokio::test]
async fn a_call_with_no_context_gets_one_created_and_written_back() {
    let Some(pool) = try_pool().await else {
        return;
    };
    ensure_test_bootstrap();
    let repositories = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let mut rc = context_for(&user, &session, empty_context());

    let result = ensure_task_exists(&repositories, &mut rc, "tool", "srv")
        .await
        .expect("a context is created");

    assert!(
        !rc.context_id().as_str().is_empty(),
        "the created context id must be written back into the request context"
    );
    assert!(
        result.is_owner,
        "a freshly created task belongs to this call"
    );
    assert_eq!(
        rc.task_id(),
        Some(&result.task_id),
        "the new task id must also be written back"
    );
}

// Why: MCP sessions are long-lived and make many tool calls. Each one must
// land in the same context or a single conversation fragments into one context
// per call, and the user's history stops being a history.
#[tokio::test]
async fn two_calls_in_one_session_land_in_the_same_context() {
    let Some(pool) = try_pool().await else {
        return;
    };
    ensure_test_bootstrap();
    let repositories = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;

    let mut first = context_for(&user, &session, empty_context());
    ensure_task_exists(&repositories, &mut first, "tool", "srv")
        .await
        .expect("first call");
    let first_context = first.context_id().clone();

    let mut second = context_for(&user, &session, empty_context());
    ensure_task_exists(&repositories, &mut second, "tool", "srv")
        .await
        .expect("second call");

    assert_eq!(
        second.context_id(),
        &first_context,
        "the session's existing context must be reused, not duplicated"
    );
}

// Why: a context id the caller owns is theirs to keep. Replacing it would
// scatter one conversation across contexts for no reason.
#[tokio::test]
async fn a_context_the_caller_owns_is_kept() {
    let Some(pool) = try_pool().await else {
        return;
    };
    ensure_test_bootstrap();
    let repositories = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;

    let mut seeding = context_for(&user, &session, empty_context());
    ensure_task_exists(&repositories, &mut seeding, "tool", "srv")
        .await
        .expect("seed a context");
    let owned = seeding.context_id().clone();

    let mut rc = context_for(&user, &session, owned.clone());
    ensure_task_exists(&repositories, &mut rc, "tool", "srv")
        .await
        .expect("owned context is accepted");

    assert_eq!(
        rc.context_id(),
        &owned,
        "an owned context must be used as given"
    );
}

// Why: this is the ownership boundary. `context_id` arrives from the caller,
// so a client can name any context it likes. Writing this call's task into a
// context belonging to another user would leak one user's MCP activity into
// another user's history. The resolver must refuse the supplied id and
// substitute a fresh one rather than failing open OR failing the call.
#[tokio::test]
async fn a_context_belonging_to_another_user_is_replaced_not_used() {
    let Some(pool) = try_pool().await else {
        return;
    };
    ensure_test_bootstrap();
    let repositories = repos(&pool);
    let (owner, owner_session) = seed_user_and_session(&pool).await;
    let (intruder, intruder_session) = seed_user_and_session(&pool).await;

    let mut owner_ctx = context_for(&owner, &owner_session, empty_context());
    ensure_task_exists(&repositories, &mut owner_ctx, "tool", "srv")
        .await
        .expect("owner seeds a context");
    let victim_context = owner_ctx.context_id().clone();

    let mut rc = context_for(&intruder, &intruder_session, victim_context.clone());
    ensure_task_exists(&repositories, &mut rc, "tool", "srv")
        .await
        .expect("the call still succeeds with a substituted context");

    assert_ne!(
        rc.context_id(),
        &victim_context,
        "another user's context must never be adopted"
    );
    assert!(
        !rc.context_id().as_str().is_empty(),
        "a replacement context must actually be created"
    );
}

// Why: the same substitution has to happen for an id that names nothing at
// all, which is what a stale or fabricated client id looks like. Failing the
// call instead would make every expired context a hard error for the user.
#[tokio::test]
async fn a_context_that_does_not_exist_is_replaced_rather_than_failing_the_call() {
    let Some(pool) = try_pool().await else {
        return;
    };
    ensure_test_bootstrap();
    let repositories = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let ghost = ContextId::generate();

    let mut rc = context_for(&user, &session, ghost.clone());
    let result = ensure_task_exists(&repositories, &mut rc, "tool", "srv")
        .await
        .expect("an unknown context must not fail the call");

    assert_ne!(
        rc.context_id(),
        &ghost,
        "an unknown context id must be replaced"
    );
    assert!(result.is_owner);
}
