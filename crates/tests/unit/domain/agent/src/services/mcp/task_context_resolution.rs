//! How an MCP tool call acquires the task and context it runs under.
//!
//! `ensure_task_exists` is the entry point every MCP tool call passes through.
//! It decides whether a parent task is inherited rather than duplicated, and —
//! the security-relevant part — what happens when the caller supplies a context
//! id belonging to somebody else.
//!
//! The resolver's other branch, for an empty `context_id`, is not covered here
//! because it cannot be reached: `ContextId` is a validated UUID-v4 id, and
//! `try_new`, `new_unchecked` and its `Deserialize` impl all route through the
//! same validator, so no `ContextId` can hold an empty string.

use systemprompt_agent::services::mcp::task_helper::ensure_task_exists;
use systemprompt_identifiers::{
    Actor, AgentName, ContextId, SessionId, TaskId, TraceId, UserId,
};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_test_fixtures::ensure_test_bootstrap;

use crate::repository::{repos, seed_user_and_session, try_pool_or_skip};

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

// Seeds a context the given user genuinely owns, by going through the
// repository rather than through the resolver under test.
async fn owned_context(
    repositories: &systemprompt_agent::repository::A2ARepositories,
    user: &UserId,
    session: &SessionId,
) -> ContextId {
    systemprompt_agent::repository::ContextRepository::new(repositories.db_pool())
        .expect("context repo")
        .create_context(
            user,
            Some(session),
            "seeded",
            systemprompt_agent::models::context::ContextKind::User,
        )
        .await
        .expect("seed context")
}

// Why: a nested tool call inherits its parent's task. Minting a second task
// would split one logical operation into two rows and make `is_owner` a lie —
// the nested caller would go on to close a task it does not own.
#[tokio::test]
async fn a_call_that_already_carries_a_task_reuses_it_and_disclaims_ownership() {
    let Some(pool) = try_pool_or_skip().await else {
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

// Why: a context id the caller owns is theirs to keep. Replacing it would
// scatter one conversation across contexts for no reason.
#[tokio::test]
async fn a_context_the_caller_owns_is_kept() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    ensure_test_bootstrap();
    let repositories = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;

    let owned = owned_context(&repositories, &user, &session).await;

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
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    ensure_test_bootstrap();
    let repositories = repos(&pool);
    let (owner, owner_session) = seed_user_and_session(&pool).await;
    let (intruder, intruder_session) = seed_user_and_session(&pool).await;

    let victim_context = owned_context(&repositories, &owner, &owner_session).await;

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
    let Some(pool) = try_pool_or_skip().await else {
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
