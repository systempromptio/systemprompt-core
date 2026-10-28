//! Every read view answers alike over `GET /{id}/read` and the
//! `documents_read` tool, and a refused read carries the same code.

use std::sync::Arc;

use anyhow::Result;
use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode, header};
use axum::{Extension, Router};
use serde_json::{Value, json};
use systemprompt_api::routes::documents::{DocumentsState, document_service, router};
use systemprompt_identifiers::{
    DocumentId, DocumentNodeId, DocumentRevisionId, McpExecutionId, McpServerId, UserId,
};
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::auth::{AuthenticatedUser, Permission};
use systemprompt_runtime::AppContext;
use systemprompt_runtime::documents::mcp::{
    DocumentsMcpConfig, DocumentsMcpTools, ReadHandler, refusal_result,
};
use systemprompt_runtime::documents::views::{NodeRange, ReadInput, ReadView};
use systemprompt_test_fixtures::{AuthedFixture, seed_bridge_credential};
use tower::ServiceExt;
use uuid::Uuid;

use super::common::{request_context, setup_ctx};

async fn send(app: Router, req: Request<Body>) -> Result<(StatusCode, Value)> {
    let resp = app.oneshot(req).await?;
    let status = resp.status();
    let body = to_bytes(resp.into_body(), 16 * 1024 * 1024).await?;
    Ok((status, serde_json::from_slice(&body).unwrap_or(Value::Null)))
}

fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(Body::empty())
        .expect("request")
}

fn tools(ctx: &Arc<AppContext>) -> DocumentsMcpTools {
    DocumentsMcpTools::new(DocumentsMcpConfig {
        service: document_service(ctx),
        users: Arc::clone(ctx.user_repository()),
        tokens: ctx.token_authority(),
        server_name: McpServerId::new("documents"),
        api_external_url: "http://localhost".to_owned(),
    })
}

fn input(document: &str, view: ReadView) -> ReadInput {
    ReadInput {
        document_id: DocumentId::new(document),
        view,
        range: None,
        sheet: None,
        cells: None,
        revision_id: None,
        since_revision: None,
        node_id: None,
        text: None,
        verbose: false,
    }
}

struct Case {
    query: String,
    input: ReadInput,
}

fn cases(document: &str, base: &str, node: &str) -> Vec<Case> {
    let node_id = DocumentNodeId::new(node);
    let mut out = Vec::new();
    for (query, view) in [
        ("view=outline", ReadView::Outline),
        ("view=text", ReadView::Text),
        ("view=cells&cells=A1:B2", ReadView::Cells),
    ] {
        let mut read = input(document, view);
        if view == ReadView::Cells {
            read.cells = Some("A1:B2".to_owned());
        }
        out.push(Case {
            query: query.to_owned(),
            input: read,
        });
    }
    let mut changes = input(document, ReadView::Changes);
    changes.since_revision = Some(DocumentRevisionId::new(base));
    out.push(Case {
        query: format!("view=changes&since_revision={base}"),
        input: changes,
    });
    let mut by_node = input(document, ReadView::Node);
    by_node.node_id = Some(node_id.clone());
    out.push(Case {
        query: format!("view=node&node_id={node}"),
        input: by_node,
    });
    let mut search = input(document, ReadView::Search);
    search.text = Some("plan".to_owned());
    out.push(Case {
        query: "view=search&text=plan".to_owned(),
        input: search,
    });
    let mut range = input(document, ReadView::Range);
    range.range = Some(NodeRange {
        from_node: node_id.clone(),
        to_node: node_id,
    });
    out.push(Case {
        query: format!("view=range&from_node={node}&to_node={node}"),
        input: range,
    });
    out.push(Case {
        query: "view=meta".to_owned(),
        input: input(document, ReadView::Meta),
    });
    out
}

async fn create(state: &DocumentsState, owner: &AuthedFixture) -> Result<(String, String)> {
    let body = json!({
        "template_id": "blank-markdown",
        "name": format!("parity-{}.md", Uuid::new_v4().simple()),
        "params": { "values": { "title": "Plan" } }
    });
    let request = Request::builder()
        .method(Method::POST)
        .uri("/")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))?;
    let (status, created) = send(rest(state, owner), request).await?;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    Ok((
        created["document"]["id"].as_str().expect("id").to_owned(),
        created["revision"]["id"].as_str().expect("rev").to_owned(),
    ))
}

fn rest(state: &DocumentsState, user: &AuthedFixture) -> Router {
    router(state.clone()).layer(Extension(request_context(user.user_id.as_str())))
}

fn first_node(read: &Value) -> String {
    let line = read["lines"][0].as_str().expect("a line");
    let (anchor, _) = line.split_once('|').expect("hashline");
    anchor.rsplit_once(':').expect("node:hash").0.to_owned()
}

#[tokio::test]
async fn every_read_view_answers_alike_over_rest_and_mcp() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let state = DocumentsState::build(&ctx)?;
    let owner = seed_bridge_credential(&pool, "docs-read-parity").await?;
    let (document, base) = create(&state, &owner).await?;
    let (_, outline) = send(
        rest(&state, &owner),
        get(&format!("/{document}/read?view=outline")),
    )
    .await?;
    let node = first_node(&outline);
    let who = AuthenticatedUser::new_with_roles(
        UserId::new(owner.user_id.as_str()),
        "parity".to_owned(),
        "parity@example.com".to_owned(),
        vec![Permission::User],
        Vec::new(),
    );
    let mcp_ctx = request_context(owner.user_id.as_str()).with_user(who);
    let tools = tools(&ctx);
    for case in cases(&document, &base, &node) {
        let (status, over_rest) = send(
            rest(&state, &owner),
            get(&format!("/{document}/read?{}", case.query)),
        )
        .await?;
        assert_eq!(status, StatusCode::OK, "{}: {over_rest}", case.query);
        let (over_mcp, _) = ReadHandler { tools: &tools }
            .handle(case.input, &mcp_ctx, &McpExecutionId::generate())
            .await
            .map_err(|error| anyhow::anyhow!("{}: {error:?}", case.query))?;
        assert_eq!(
            over_rest,
            serde_json::to_value(&over_mcp)?,
            "{}",
            case.query
        );
    }

    let (status, refused) = send(
        rest(&state, &owner),
        get(&format!("/{document}/read?view=node&node_id=missing.1")),
    )
    .await?;
    assert_eq!(status, StatusCode::CONFLICT, "{refused}");
    let mut missing = input(&document, ReadView::Node);
    missing.node_id = Some(DocumentNodeId::new("missing.1"));
    let error = ReadHandler { tools: &tools }
        .handle(missing, &mcp_ctx, &McpExecutionId::generate())
        .await
        .expect_err("unknown node");
    let mcp = refusal_result(&error)
        .and_then(|r| r.structured_content)
        .expect("refusal");
    assert_eq!(refused["error_key"], mcp["code"]);
    assert_eq!(refused["detail"], mcp);
    Ok(())
}
