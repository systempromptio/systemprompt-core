use systemprompt_governance::authz::types::AuthzContext;
use systemprompt_identifiers::{DocumentFolderId, DocumentId, ModelId};
use systemprompt_models::documents::DocumentAction;

const PARENT: &str = "00000000-0000-4000-8000-00000000f001";
const ROOT: &str = "00000000-0000-4000-8000-00000000f002";

fn folders(ids: &[&str]) -> Vec<DocumentFolderId> {
    ids.iter().map(|id| DocumentFolderId::new(*id)).collect()
}

#[test]
fn ancestors_round_trip_in_order() {
    let ctx = AuthzContext::document_access(DocumentAction::Edit, None, None)
        .with_folder_ancestors(&folders(&[PARENT, ROOT]));
    let ancestors = ctx.document_access_ancestors().expect("ancestors");
    assert_eq!(ancestors, folders(&[PARENT, ROOT]));
    assert_eq!(ctx.document_action(), Some(DocumentAction::Edit));
}

#[test]
fn empty_ancestors_leave_payload_untouched() {
    let ctx = AuthzContext::document_access(DocumentAction::View, None, None);
    let before = ctx.payload.clone();
    let ctx = ctx.with_folder_ancestors(&[]);
    assert_eq!(ctx.payload, before);
    assert!(ctx.document_access_ancestors().expect("none").is_empty());
}

#[test]
fn ancestors_are_ignored_on_other_kinds() {
    let ctx = AuthzContext::gateway_invocation(&ModelId::new("model-a"), None)
        .with_folder_ancestors(&folders(&[PARENT]));
    assert!(ctx.payload.get("folder_ancestors").is_none());
    assert!(ctx.document_access_ancestors().expect("none").is_empty());
    assert_eq!(ctx.document_action(), None);
    assert_eq!(ctx.document_access_document(), None);
    assert_eq!(ctx.document_access_folder(), None);
}

#[test]
fn non_array_ancestors_are_refused() {
    let ctx = AuthzContext::extension(
        AuthzContext::DOCUMENT_ACCESS_KIND,
        serde_json::json!({ "action": "view", "folder_ancestors": PARENT }),
    );
    let err = ctx.document_access_ancestors().expect_err("malformed");
    assert!(err.to_string().contains("not an array"));
}

#[test]
fn non_string_ancestor_entry_is_refused() {
    let ctx = AuthzContext::extension(
        AuthzContext::DOCUMENT_ACCESS_KIND,
        serde_json::json!({ "action": "view", "folder_ancestors": [PARENT, 7] }),
    );
    let err = ctx.document_access_ancestors().expect_err("malformed");
    assert!(err.to_string().contains("not a string"));
}

#[test]
fn empty_string_ancestor_entry_is_refused() {
    let ctx = AuthzContext::extension(
        AuthzContext::DOCUMENT_ACCESS_KIND,
        serde_json::json!({ "action": "view", "folder_ancestors": [""] }),
    );
    assert!(ctx.document_access_ancestors().is_err());
}

#[test]
fn unknown_action_and_non_string_targets_read_as_none() {
    let ctx = AuthzContext::extension(
        AuthzContext::DOCUMENT_ACCESS_KIND,
        serde_json::json!({ "action": "teleport", "document_id": 5, "folder_id": "" }),
    );
    assert_eq!(ctx.document_action(), None);
    assert_eq!(ctx.document_access_document(), None);
    assert_eq!(ctx.document_access_folder(), None);
}

#[test]
fn targets_are_read_back() {
    let doc = DocumentId::new("00000000-0000-4000-8000-00000000d001");
    let folder = DocumentFolderId::new(ROOT);
    let ctx = AuthzContext::document_access(DocumentAction::Share, Some(&doc), Some(&folder));
    assert_eq!(ctx.document_access_document(), Some(doc));
    assert_eq!(ctx.document_access_folder(), Some(folder));
    assert_eq!(ctx.document_action(), Some(DocumentAction::Share));
}
