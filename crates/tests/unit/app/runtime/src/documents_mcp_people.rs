use systemprompt_mcp::McpToolHandler;
use systemprompt_models::documents::AccessLevel;
use systemprompt_runtime::documents::mcp::{PeopleHandler, PeopleInput, ShareHandler, ShareInput};
use systemprompt_runtime::documents::refusal::RefusalCode;

use crate::documents_mcp_rig::code_of;
use crate::documents_mcp_support::{create_text, ctx_for, exec, harness, registered};

fn query(text: &str) -> PeopleInput {
    PeopleInput {
        query: text.to_owned(),
        document_id: None,
        limit: None,
    }
}

#[tokio::test]
async fn a_blank_query_is_refused_as_invalid_params() {
    let h = harness().await;
    let caller = registered("people-blank").await;

    let refused = PeopleHandler { tools: &h.tools }
        .handle(query("   "), &ctx_for(Some(&caller)), &exec())
        .await
        .expect_err("blank query refused");

    assert_eq!(code_of(&refused), RefusalCode::InvalidParams);
}

#[tokio::test]
async fn people_are_found_by_id_prefix_with_their_email() {
    let h = harness().await;
    let caller = registered("people-caller").await;
    let target = registered("people-target").await;

    let (found, summary) = PeopleHandler { tools: &h.tools }
        .handle(query(target.id.as_str()), &ctx_for(Some(&caller)), &exec())
        .await
        .expect("search");

    assert_eq!(summary, format!("{} people", found.people.len()));
    let person = found
        .people
        .iter()
        .find(|person| person.user_id == target.id)
        .expect("target listed");
    assert_eq!(person.email.as_deref(), Some(target.email.as_str()));
    assert_eq!(person.access, None);
}

#[tokio::test]
async fn with_a_document_each_match_carries_its_level_there() {
    let h = harness().await;
    let owner = registered("people-owner").await;
    let editor = registered("people-editor").await;
    let ctx = ctx_for(Some(&owner));
    let (document, _) = create_text(&h.tools, &ctx, "body\n").await;
    ShareHandler { tools: &h.tools }
        .handle(
            ShareInput {
                document_id: Some(document.clone()),
                folder_id: None,
                subject_kind: Some("user".to_owned()),
                subject_value: Some(editor.id.as_str().to_owned()),
                level: Some(AccessLevel::Edit),
                valid_until: None,
                transfer_to: None,
                revoke_all: false,
                message: None,
            },
            &ctx,
            &exec(),
        )
        .await
        .expect("share");

    let (found, _) = PeopleHandler { tools: &h.tools }
        .handle(
            PeopleInput {
                query: editor.id.as_str().to_owned(),
                document_id: Some(document),
                limit: Some(5),
            },
            &ctx,
            &exec(),
        )
        .await
        .expect("search on document");

    let person = found
        .people
        .iter()
        .find(|person| person.user_id == editor.id)
        .expect("editor listed");
    assert_eq!(person.access, Some(AccessLevel::Edit));
}
