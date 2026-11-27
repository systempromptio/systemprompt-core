use sp_protocol::wire::drive::{
    CommentRequest, DriveAgentCap, DriveComment, DriveContext, DriveDetail, DriveDocument,
    DriveErrorBody, DriveLanded, DriveListing, DriveScope, DriveUpload, EditMode, EditRequest,
    OutlineLine, parse_outline_line,
};

const DOC: &str = "6f1c2b8e-1d2a-4c3b-9e8f-0a1b2c3d4e5f";
const FOLDER: &str = "0b1c2d3e-4f50-4617-8293-a4b5c6d7e8f9";
const REV: &str = "11111111-2222-4333-8444-555555555555";
const USER: &str = "1ecce4e3-bc87-441d-b374-ea29bab88da8";

fn document() -> serde_json::Value {
    serde_json::json!({
        "id": DOC, "folder_id": null, "name": "Q3 report", "format": "html", "mime_type": "text/html",
        "head_revision_id": REV, "template_id": "astound-word", "owner_user_id": USER,
        "created_at": "2026-11-01T09:00:00Z", "updated_at": "2026-11-01T09:00:00Z",
        "size_bytes": 1234, "my_access": "own", "future": 1
    })
}

fn revision() -> serde_json::Value {
    serde_json::json!({
        "id": REV, "document_id": DOC, "parent_revision_id": null, "content_sha256": "abc",
        "author_user_id": USER, "actor_kind": "agent", "origin_kind": "api", "message": null,
        "created_at": "2026-11-01T09:00:00Z"
    })
}

#[test]
fn listings_and_details_decode() {
    let listing: DriveListing = serde_json::from_value(serde_json::json!({
        "folders": [{
            "id": FOLDER, "parent_id": null, "name": "Reports", "owner_user_id": USER,
            "agent_edit_policy": "suggest", "created_at": "t", "updated_at": "t"
        }],
        "documents": [document()],
        "next_cursor": null
    }))
    .unwrap();
    assert_eq!(listing.documents[0].folder_id, None);
    assert_eq!(
        listing.folders[0].agent_edit_policy.as_deref(),
        Some("suggest")
    );
    let folders_only: DriveListing =
        serde_json::from_value(serde_json::json!({ "folders": [] })).unwrap();
    assert_eq!(folders_only.documents, Vec::<DriveDocument>::new());
    let detail: DriveDetail = serde_json::from_value(serde_json::json!({
        "document": document(), "head": revision(), "my_access": "own", "folder_path": null
    }))
    .unwrap();
    assert_eq!(detail.head.actor_kind, "agent");
}

#[test]
fn ids_that_are_not_uuids_fail() {
    let mut value = document();
    value["id"] = serde_json::json!("d1");
    assert!(serde_json::from_value::<DriveDocument>(value).is_err());
}

#[test]
fn a_context_names_a_uuid() {
    const CONTEXT: &str = "5d1e7c2a-8b3f-4a6d-9e0c-1f2a3b4c5d6e";
    let context: DriveContext = serde_json::from_value(
        serde_json::json!({ "data": { "context_id": CONTEXT, "name": "Notes" } }),
    )
    .unwrap();
    assert_eq!(context.data.context_id.as_str(), CONTEXT);
    assert_eq!(context.data.name, "Notes");
    let refused = serde_json::json!({ "data": { "context_id": "ctx-1", "name": "Notes" } });
    assert!(serde_json::from_value::<DriveContext>(refused).is_err());
}

#[test]
fn edits_land_or_propose() {
    let landed: DriveLanded =
        serde_json::from_value(serde_json::json!({ "revision": revision(), "merged_from": null }))
            .unwrap();
    assert!(landed.merged_from.is_none());
    let request = EditRequest {
        ops: vec![serde_json::json!({ "op": "insert_after", "node": "n1", "text": "x" })],
        base_revision: None,
        mode: Some(EditMode::Propose),
        op_id: Some("op-1".to_owned()),
        message: Some("Report Q3".to_owned()),
    };
    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["mode"], "propose");
    assert!(json.get("base_revision").is_none());
}

#[test]
fn comments_and_requests() {
    let comment: DriveComment = serde_json::from_value(serde_json::json!({
        "id": "22222222-3333-4444-8555-666666666666", "anchor": { "node": "n1", "find": "Hola" },
        "author_user_id": USER, "body": "Check this", "parent_id": null, "created_at": "t",
        "resolved_at": null, "resolved_by": null
    }))
    .unwrap();
    assert_eq!(comment.anchor.map(|a| a.node), Some("n1".to_owned()));
    let reply = CommentRequest {
        body: "ok".to_owned(),
        anchor: None,
        parent_id: Some(comment.id),
        mention_options: Vec::new(),
    };
    let json = serde_json::to_value(&reply).unwrap();
    assert!(json.get("anchor").is_none() && json.get("mention_options").is_none());
    assert_eq!(json["parent_id"], "22222222-3333-4444-8555-666666666666");
}

#[test]
fn uploads_and_parts() {
    let upload: DriveUpload = serde_json::from_value(serde_json::json!({
        "upload_id": "33333333-4444-4555-8666-777777777777", "name": "big.pdf", "mime": "application/pdf",
        "size": 20_000_000u64, "sha256": "abc", "part_size": 8_388_608u64, "part_count": 3, "status": "open",
        "received_bytes": 0, "received_parts": [], "missing_parts": [1, 2, 3], "blob_ref": null,
        "expires_at": "t"
    }))
    .unwrap();
    assert_eq!(upload.part_range(1), Some(0..8_388_608));
    assert_eq!(upload.part_range(3), Some(16_777_216..20_000_000));
    assert_eq!(upload.part_range(0), None);
    assert_eq!(upload.part_range(4), None);
}

#[test]
fn outline_lines_parse() {
    assert_eq!(
        parse_outline_line("n1:3639efcd| # Hola"),
        Some(OutlineLine {
            node: "n1",
            hash: "3639efcd",
            text: "# Hola"
        })
    );
    assert_eq!(
        parse_outline_line("n2:e3b0c442| ").map(|l| l.text),
        Some("")
    );
    assert_eq!(parse_outline_line("garbage"), None);
}

#[test]
fn conflict_bodies_and_agent_refusals() {
    let body: DriveErrorBody = serde_json::from_value(serde_json::json!({
        "message": "lost write", "error_key": "conflict", "detail": { "head_revision_id": REV }
    }))
    .unwrap();
    assert_eq!(body.head_revision_id(), Some(REV));
    let refusal: DriveErrorBody =
        serde_json::from_value(serde_json::json!({ "error_key": "agent_suggest_only" })).unwrap();
    assert!(refusal.is_agent_refusal());
}

#[test]
fn agent_caps_decode() {
    let reading = serde_json::json!({ "agent_cap": "edit", "policy_cap": "edit", "source": { "kind": "policy", "policy": "default" }, "reason": null });
    let cap: DriveAgentCap = serde_json::from_value(serde_json::json!({
        "target_kind": "folder", "target_id": FOLDER, "override": null, "agent_cap": "suggest",
        "policy_cap": "suggest", "source": { "kind": "policy" }, "reason": null,
        "attended": reading, "unattended": { "agent_cap": "suggest", "policy_cap": "suggest", "source": { "kind": "policy" } }
    }))
    .unwrap();
    assert_eq!(cap.attended.agent_cap, "edit");
    assert_eq!(DriveScope::Folder.as_str(), "folder");
}
