use std::sync::Arc;

use serde_json::json;
use systemprompt_brain::BrainError;
use systemprompt_brain::models::FactFilter;
use systemprompt_identifiers::{AgentName, ContextId, McpExecutionId, SessionId, TraceId, UserId};
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::auth::{AuthenticatedUser, Permission};
use systemprompt_models::brain::FactScope;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_runtime::brain::mcp::{BrainMcpTools, SearchHandler};

use crate::events::entered;
use crate::harness::{Harness, Setup};
use crate::plane::{PRICING, kind};

async fn enter(h: &Harness, who: &UserId, product: &str) {
    h.emit(&entered(
        who,
        kind(PRICING),
        product,
        serde_json::json!({"price": "1200", "region": "emea"}),
        FactScope::User(who.clone()),
    ))
    .await;
    h.drain().await;
}

async fn walk(h: &Harness, who: &UserId, limit: i64) -> Vec<String> {
    let caller = Harness::caller(who, &[]);
    let mut seen = Vec::new();
    let mut after = None;
    loop {
        let mut filter = FactFilter::new().with_limit(limit);
        if let Some(token) = after.take() {
            filter = filter.with_after(token);
        }
        let page = h.service.search(&caller, &filter).await.expect("page");
        seen.extend(page.facts.iter().map(|fact| fact.subject.id.clone()));
        match page.next {
            Some(token) => after = Some(token),
            None => return seen,
        }
    }
}

fn ctx(user: &UserId) -> RequestContext {
    let person = AuthenticatedUser::new(
        user.clone(),
        user.as_str().to_owned(),
        format!("{}@test.invalid", user.as_str()),
        vec![Permission::User],
    );
    RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::try_new("brain-paging-test").expect("agent name"),
        systemprompt_identifiers::Actor::user(user.clone()),
    )
    .with_user(person)
}

async fn mcp_walk(h: &Harness, who: &UserId) -> Vec<String> {
    let tools = BrainMcpTools::from_parts(
        Arc::new(h.service.clone()),
        Arc::new(h.recall()),
        Arc::new(h.profile.clone()),
    )
    .expect("enabled");
    let ctx = ctx(who);
    let mut seen = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let input = json!({"query": "", "limit": 1, "after": after});
        let (out, _) = SearchHandler { tools: &tools }
            .handle(
                serde_json::from_value(input).expect("input"),
                &ctx,
                &McpExecutionId::generate(),
            )
            .await
            .expect("search");
        seen.extend(out.facts.iter().map(|fact| fact.subject.clone()));
        match out.next {
            Some(token) => after = Some(token),
            None => return seen,
        }
    }
}

#[tokio::test]
async fn a_listing_is_never_spent_on_facts_the_caller_cannot_reach() {
    let h = Harness::with("brain_search_paging", Setup::confirming()).await;
    let owner = h.user("owner", &[]).await;
    let other = h.user("other", &[]).await;
    enter(&h, &owner, "acme").await;
    for product in ["globex", "initech", "hooli"] {
        enter(&h, &other, product).await;
    }

    let mine = h
        .service
        .search(
            &Harness::caller(&owner, &[]),
            &FactFilter::new().with_limit(1),
        )
        .await
        .expect("owner search");
    assert_eq!(mine.facts.len(), 1);
    assert_eq!(mine.facts[0].subject.id, "acme");
    h.finish().await;
}

#[tokio::test]
async fn paging_walks_each_visible_fact_once_and_skips_a_hidden_one_between_them() {
    let h = Harness::with("brain_search_walk", Setup::confirming()).await;
    let a = h.user("a", &[]).await;
    let b = h.user("b", &[]).await;
    enter(&h, &b, "globex").await;
    enter(&h, &a, "acme").await;
    enter(&h, &b, "initech").await;
    enter(&h, &a, "umbrella").await;
    enter(&h, &b, "hooli").await;

    for limit in [1, 2, 3] {
        let mut seen = walk(&h, &b, limit).await;
        seen.sort();
        assert_eq!(seen, ["globex", "hooli", "initech"], "limit {limit}");
    }
    let mut owner_seen = walk(&h, &a, 1).await;
    owner_seen.sort();
    assert_eq!(owner_seen, ["acme", "umbrella"], "negative control");

    let mut tool_seen = mcp_walk(&h, &b).await;
    tool_seen.sort();
    assert_eq!(
        tool_seen,
        ["product:globex", "product:hooli", "product:initech"]
    );

    let a_page = h
        .service
        .search(&Harness::caller(&a, &[]), &FactFilter::new().with_limit(1))
        .await
        .expect("a page");
    let token = a_page.next.expect("a full page carries a cursor");
    let refused = h
        .service
        .search(
            &Harness::caller(&b, &[]),
            &FactFilter::new().with_limit(1).with_after(token),
        )
        .await;
    assert!(matches!(refused, Err(BrainError::Cursor(_))), "{refused:?}");
    h.finish().await;
}
