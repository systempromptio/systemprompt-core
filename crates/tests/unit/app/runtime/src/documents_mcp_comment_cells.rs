use serde_json::json;
use systemprompt_documents::mcp::{
    CommentHandler, CommentInput, CreateHandler, CreateInput, DocumentsMcpTools,
};
use systemprompt_documents::refusal::RefusalCode;
use systemprompt_documents::views::CommentOutput;
use systemprompt_extension::mcp::McpToolHandler;
use systemprompt_identifiers::{DocumentId, DocumentTemplateId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{CellRange, NodeKind};
use systemprompt_models::execution::context::RequestContext;
use uuid::Uuid;

use crate::documents_mcp_rig::{code_of, rig};
use crate::documents_mcp_support::{create_text, ctx_for, exec, line_for, read_text, registered};

fn input(document: &DocumentId, extra: serde_json::Value) -> CommentInput {
    let mut value = json!({ "document_id": document, "action": "create" });
    for (key, field) in extra.as_object().expect("object") {
        value[key] = field.clone();
    }
    serde_json::from_value(value).expect("comment input")
}

async fn spreadsheet(
    tools: &DocumentsMcpTools,
    ctx: &RequestContext,
    who: &AuthenticatedUser,
) -> (DocumentId, String) {
    let document = CreateHandler { tools }
        .handle(
            CreateInput {
                template_id: DocumentTemplateId::new("blank-xlsx"),
                folder_id: None,
                name: format!("figures-{}", Uuid::new_v4().simple()),
                params: Default::default(),
                content: None,
                ops: Vec::new(),
                message: None,
                context_id: None,
            },
            ctx,
            &exec(),
        )
        .await
        .expect("create workbook")
        .0
        .document_id;
    let (_, projection) = tools
        .service()
        .project(who, &document, None, ctx.trace_id())
        .await
        .expect("project");
    let sheet = projection
        .nodes()
        .find(|node| node.kind == NodeKind::Sheet)
        .expect("a sheet node")
        .id
        .as_str()
        .to_owned();
    (document, sheet)
}

async fn call(
    tools: &DocumentsMcpTools,
    ctx: &RequestContext,
    input: CommentInput,
) -> Result<CommentOutput, rmcp::ErrorData> {
    CommentHandler { tools }
        .handle(input, ctx, &exec())
        .await
        .map(|(output, _)| output)
}

#[tokio::test]
async fn a_cell_range_comment_round_trips_through_the_tool() {
    let r = rig(false).await;
    let owner = registered("cells-mcp").await;
    let ctx = ctx_for(Some(&owner));
    let (document, sheet) = spreadsheet(&r.tools, &ctx, &owner).await;
    let created = call(
        &r.tools,
        &ctx,
        input(
            &document,
            json!({"node": sheet, "cells": "b2:d9", "body": "totals?"}),
        ),
    )
    .await
    .expect("create");
    let expected: CellRange = "B2:D9".parse().expect("range");
    assert_eq!(created.comments[0].anchor.cells, Some(expected));
    let wire = serde_json::to_value(&created).expect("output json");
    assert_eq!(wire["comments"][0]["anchor"]["cells"], "B2:D9");
    assert!(wire["comments"][0]["anchor"].get("find").is_none());

    let listed = call(&r.tools, &ctx, input(&document, json!({"action": "list"})))
        .await
        .expect("list");
    assert_eq!(listed.comments.len(), 1);
    assert_eq!(listed.comments[0].anchor.cells, Some(expected));
}

#[tokio::test]
async fn a_cell_range_beside_a_quote_or_off_a_sheet_is_invalid_params() {
    let r = rig(false).await;
    let owner = registered("cells-mcp-refused").await;
    let ctx = ctx_for(Some(&owner));
    let (document, sheet) = spreadsheet(&r.tools, &ctx, &owner).await;
    let with_quote = call(
        &r.tools,
        &ctx,
        input(
            &document,
            json!({"node": sheet, "cells": "B2", "anchor": {"find": "x"}, "body": "x"}),
        ),
    )
    .await
    .expect_err("cells with a quote");
    assert_eq!(code_of(&with_quote), RefusalCode::InvalidParams);

    let (text, _) = create_text(&r.tools, &ctx, "alpha").await;
    let read = read_text(&r.tools, &ctx, &text).await;
    let node = line_for(&read, "alpha").0.to_owned();
    let off_sheet = call(
        &r.tools,
        &ctx,
        input(&text, json!({"node": node, "cells": "B2", "body": "x"})),
    )
    .await
    .expect_err("cells on text");
    assert_eq!(code_of(&off_sheet), RefusalCode::InvalidParams);
}

#[test]
fn a_malformed_cell_range_does_not_parse_as_input() {
    for bad in ["B0", "D9:B2", "XFE1", "B2:", "cells"] {
        let value =
            json!({"action": "create", "document_id": DocumentId::generate(), "cells": bad});
        assert!(
            serde_json::from_value::<CommentInput>(value).is_err(),
            "{bad} parsed"
        );
    }
}
