use serde_json::json;
use sp_client::ClientError;
use sp_drive::view::{DriveFailure, comment_threads, folder_tree};
use sp_drive::{DriveError, NeededCapability};
use sp_protocol::wire::drive::{DriveComment, DriveFolder};

use crate::support::USER;

const A: &str = "00000000-0000-4000-8000-00000000000a";
const B: &str = "00000000-0000-4000-8000-00000000000b";
const C: &str = "00000000-0000-4000-8000-00000000000c";
const D: &str = "00000000-0000-4000-8000-00000000000d";

fn folder(id: &str, parent: Option<&str>, name: &str) -> DriveFolder {
    serde_json::from_value(json!({
        "id": id, "parent_id": parent, "name": name, "owner_user_id": USER,
        "created_at": "t", "updated_at": "t"
    }))
    .unwrap()
}

fn comment(id: &str, parent: Option<&str>, at: &str) -> DriveComment {
    serde_json::from_value(json!({
        "id": id, "author_user_id": USER, "body": "b", "parent_id": parent, "created_at": at
    }))
    .unwrap()
}

#[test]
fn folders_nest_under_visible_parents() {
    let tree = folder_tree(&[
        folder(A, None, "Reports"),
        folder(B, Some(A), "2026"),
        folder(C, Some(D), "Shared with me"),
        folder(D, Some(C), "Loop"),
    ]);
    assert_eq!(tree.len(), 2);
    let reports = tree.iter().find(|node| node.name == "Reports").unwrap();
    assert_eq!(reports.children.len(), 1);
    assert_eq!(reports.children[0].name, "2026");
    let looped = tree.iter().find(|node| node.name != "Reports").unwrap();
    assert_eq!(looped.children.len(), 1);
    assert_eq!(
        looped.children[0].children,
        Vec::<sp_drive::view::FolderNode>::new()
    );
}

#[test]
fn replies_gather_under_their_thread() {
    let threads = comment_threads(vec![
        comment(B, Some(A), "2026-11-02"),
        comment(A, None, "2026-11-01"),
        comment(C, Some(D), "2026-11-03"),
    ]);
    assert_eq!(threads.len(), 2);
    assert_eq!(threads[0].root.id.to_string(), A);
    assert_eq!(threads[0].replies.len(), 1);
    assert_eq!(threads[1].root.id.to_string(), C);
}

#[test]
fn failures_map_to_view_kinds() {
    let suggest = DriveError::AgentRefused {
        endpoint: "POST /x",
        error_key: "agent_suggest_only".to_owned(),
        capability: NeededCapability::Edit,
    };
    assert_eq!(DriveFailure::from(&suggest), DriveFailure::SuggestOnly);
    assert_eq!(
        DriveFailure::from(&DriveError::Cancelled),
        DriveFailure::Cancelled
    );
    let refused = DriveError::Client(ClientError::Unauthorized {
        endpoint: "GET /x",
        error_key: None,
    });
    assert_eq!(DriveFailure::from(&refused), DriveFailure::SignedOut);
}
