use chrono::Utc;
use rmcp::model::{CallToolResult, ContentBlock};
use systemprompt_identifiers::{DocumentRevisionId, UserId};
use systemprompt_models::documents::{ActorKind, hash8};
use systemprompt_runtime::documents::mcp::{EditOutput, HistoryOutput, RevisionItem};

fn wire_bytes<T: serde::Serialize>(summary: String, output: &T) -> usize {
    let mut result = CallToolResult::success(vec![ContentBlock::text(summary)]);
    result.structured_content = Some(serde_json::to_value(output).expect("output"));
    serde_json::to_vec(&result).expect("result").len()
}

#[test]
fn a_one_op_edit_result_stays_under_400_bytes() {
    let revision = DocumentRevisionId::generate();
    let output = EditOutput {
        uri: None,
        revision_id: revision.clone(),
        parent_revision_id: None,
        applied: 1,
        merged: false,
        replayed: false,
        written: vec![format!(
            "body/p[3]:{}| Revenue grew 12% in the quarter.",
            hash8("Revenue grew 12% in the quarter.")
        )],
    };
    let summary = format!("revision {revision}: 1 ops, 1 lines written");
    let bytes = wire_bytes(summary, &output);
    assert!(bytes < 400, "{bytes} bytes");
}

#[test]
fn a_three_revision_history_stays_under_800_bytes() {
    let author = UserId::new("user_2b6c0f6e9a7d4c1e8f3a5b7d9c1e3f5a");
    let revisions = (0..3)
        .map(|n| RevisionItem {
            uri: None,
            revision_id: DocumentRevisionId::generate(),
            parent_revision_id: None,
            author_user_id: author.clone(),
            actor_kind: ActorKind::Agent,
            message: (n > 0).then(|| "tighten the summary".to_owned()),
            content_sha256: None,
            attribution: None,
            created_at: Utc::now(),
        })
        .collect();
    let bytes = wire_bytes("3 revisions".to_owned(), &HistoryOutput { revisions });
    assert!(bytes < 800, "{bytes} bytes");
}
