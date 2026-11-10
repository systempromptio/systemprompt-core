use std::sync::Arc;

use serde_json::json;
use systemprompt_brain::models::FactFilter;
use systemprompt_brain::repository::WorkingSetRepository;
use systemprompt_identifiers::{AgentName, ContextId, McpExecutionId, SessionId, TraceId, UserId};
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::auth::{AuthenticatedUser, Permission};
use systemprompt_models::brain::{FactScope, FactStatus};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_runtime::brain::mcp::{BrainMcpTools, SearchHandler};

use crate::events::{entered, price};
use crate::harness::Harness;
use crate::plane::{PRICING, kind};
use crate::recall_support::{ids, recall};

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
        AgentName::try_new("brain-personal-test").expect("agent name"),
        systemprompt_identifiers::Actor::user(user.clone()),
    )
    .with_user(person)
}

pub(crate) async fn mcp_search(h: &Harness, user: &UserId) -> usize {
    let tools = BrainMcpTools::from_parts(
        Arc::new(h.service.clone()),
        Arc::new(h.recall()),
        Arc::new(h.profile.clone()),
    )
    .expect("enabled");
    let (out, _) = SearchHandler { tools: &tools }
        .handle(
            serde_json::from_value(json!({"query": "acme"})).expect("input"),
            &ctx(user),
            &McpExecutionId::generate(),
        )
        .await
        .expect("search");
    out.facts.len()
}

#[tokio::test]
async fn a_self_confirmed_personal_fact_under_an_everyone_topic_reaches_only_its_owner() {
    let h = Harness::new("brain_personal_scope").await;
    let owner = h.user("owner", &[]).await;
    let other = h.user("other", &[]).await;
    let mut fields = price("acme", "emea", "1200");
    fields.as_object_mut().expect("object").remove("subject");
    h.emit(&entered(
        &owner,
        kind(PRICING),
        "acme",
        fields,
        FactScope::User(owner.clone()),
    ))
    .await;
    h.drain().await;
    let fact = h.only_fact("acme").await;
    h.service
        .confirm(&Harness::caller(&owner, &[]), &fact.id)
        .await
        .expect("the owner confirms their own fact");
    let fact = h.fact(&fact.id).await;
    assert_eq!(fact.status, FactStatus::Confirmed);
    assert_eq!(fact.scope, FactScope::User(owner.clone()));
    assert!(
        !fact.topics.is_empty(),
        "the pricing kind carries its topic"
    );

    assert_eq!(
        WorkingSetRepository::keys_for(&h.pg, &fact.id)
            .await
            .expect("keys"),
        vec![format!("user:{owner}")]
    );

    assert_eq!(
        ids(&recall(&h, &owner, "acme").await),
        vec![fact.id.clone()]
    );
    assert!(ids(&recall(&h, &other, "acme").await).is_empty());

    let listed = |user: &UserId| {
        let caller = Harness::caller(user, &[]);
        let service = h.service.clone();
        async move {
            service
                .search(&caller, &FactFilter::new())
                .await
                .expect("search")
                .facts
                .len()
        }
    };
    assert_eq!(
        listed(&owner).await,
        1,
        "negative control: the owner lists it"
    );
    assert_eq!(listed(&other).await, 0);

    assert!(
        h.service
            .get(&Harness::caller(&other, &[]), &fact.id)
            .await
            .is_err()
    );
    assert_eq!(mcp_search(&h, &owner).await, 1);
    assert_eq!(mcp_search(&h, &other).await, 0);
    h.finish().await;
}
