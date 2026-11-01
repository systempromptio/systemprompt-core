use std::sync::Arc;

use serde_json::json;
use systemprompt_identifiers::{AgentName, ContextId, McpExecutionId, SessionId, TraceId, UserId};
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::auth::{AuthenticatedUser, Permission};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_runtime::brain::mcp::{
    BrainMcpTools, RecallPreviewHandler, SearchHandler, WhyHandler,
};

use crate::harness::{Harness, Setup};
use crate::recall_seed::{HINTS, PROJECT, seed};

fn tools(h: &Harness) -> BrainMcpTools {
    BrainMcpTools::from_parts(
        Arc::new(h.service.clone()),
        Arc::new(h.recall()),
        Arc::new(h.profile.clone()),
    )
    .expect("enabled")
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
        AgentName::try_new("brain-tools-test").expect("agent name"),
        systemprompt_identifiers::Actor::user(user.clone()),
    )
    .with_user(person)
}

fn parse<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> T {
    serde_json::from_value(value).expect("input")
}

#[tokio::test]
async fn search_matches_statement_or_subject_text_and_scope() {
    let h = Harness::with("brain_mcp_search", Setup::confirming()).await;
    let seeded = seed(&h).await;
    let tools = tools(&h);
    let ctx = ctx(&seeded.reader);
    let search = |value: serde_json::Value| {
        let tools = &tools;
        let ctx = &ctx;
        async move {
            SearchHandler { tools }
                .handle(parse(value), ctx, &McpExecutionId::generate())
                .await
                .expect("search")
        }
    };

    let (all, _) = search(json!({"query": ""})).await;
    assert!(all.facts.len() >= 4, "{:?}", all.facts.len());

    let (acme, summary) = search(json!({"query": "  ACME "})).await;
    assert!(!acme.facts.is_empty());
    assert_eq!(summary, format!("{} facts", acme.facts.len()));
    assert!(acme.facts.iter().all(|fact| {
        fact.statement.to_lowercase().contains("acme")
            || fact.subject.to_lowercase().contains("acme")
    }));

    let (limited, _) = search(json!({"query": "", "limit": 1})).await;
    assert_eq!(limited.facts.len(), 1);

    let (project, _) = search(json!({"query": "", "scope": "project"})).await;
    assert!(
        project
            .facts
            .iter()
            .any(|fact| fact.subject.contains("omega"))
    );
    assert!(
        !project
            .facts
            .iter()
            .any(|fact| fact.subject.contains("zeta"))
    );

    let (none, summary) = search(json!({"query": "no-such-words-here"})).await;
    assert!(none.facts.is_empty());
    assert_eq!(summary, "0 facts");
    h.finish().await;
}

#[tokio::test]
async fn why_scores_a_visible_fact() {
    let h = Harness::with("brain_mcp_why", Setup::confirming()).await;
    let seeded = seed(&h).await;
    let tools = tools(&h);
    let (why, summary) = WhyHandler { tools: &tools }
        .handle(
            parse(json!({"fact_id": seeded.subject, "project": PROJECT})),
            &ctx(&seeded.reader),
            &McpExecutionId::generate(),
        )
        .await
        .expect("why");
    assert_eq!(why.fact_id, seeded.subject);
    assert!(why.revision >= 1);
    assert_eq!(summary, format!("score {:.3}", why.total));
    h.finish().await;
}

#[tokio::test]
async fn a_recall_preview_over_seeded_facts_injects_a_block() {
    let h = Harness::with("brain_mcp_preview", Setup::confirming()).await;
    let seeded = seed(&h).await;
    let tools = tools(&h);
    let (preview, summary) = RecallPreviewHandler { tools: &tools }
        .handle(
            parse(json!({"prompt": HINTS, "project": PROJECT})),
            &ctx(&seeded.reader),
            &McpExecutionId::generate(),
        )
        .await
        .expect("preview");
    assert!(preview.injected, "{summary}");
    assert!(preview.skipped.is_none());
    assert!(preview.tokens > 0);
    let block = preview.block.expect("block");
    assert!(block.contains("acme"), "{block}");
    h.finish().await;
}
