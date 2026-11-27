use std::sync::Arc;

use harness_tools::ToolError;
use serde_json::json;
use sp_drive::extension::{PERMISSION, TOOL_NAMES};
use sp_drive::{DocumentKind, DriveExtension, DriveHandle, DriveSettings, DriveWithheld};
use sp_extension::{ExtensionId, SpExtension};

use crate::support::{FakeInstance, FakeResponse, Gate, REV, USER, applied, context, tool_context};

#[test]
fn the_extension_is_drive_and_offers_its_six_tools() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let extension = DriveExtension::default();
    assert_eq!(
        extension.metadata().id,
        ExtensionId::parse("drive").unwrap()
    );
    assert!(!extension.metadata().required);
    let engine = extension.engine(&ctx).unwrap();
    assert_eq!(engine.metadata().id, DriveExtension::ID);
    let tools = engine.tools();
    let names: Vec<&str> = tools.iter().map(|tool| tool.tool.name()).collect();
    assert_eq!(names, TOOL_NAMES);
    assert_eq!(
        TOOL_NAMES,
        [
            "drive_search",
            "drive_list",
            "drive_read",
            "drive_propose_edit",
            "drive_create",
            "drive_comment"
        ]
    );
    assert!(
        tools
            .iter()
            .all(|tool| tool.asks_itself && tool.permission == PERMISSION)
    );
    assert_eq!(PERMISSION, "drive");
    assert_eq!(extension.services(&ctx).len(), 1);
    assert_eq!(extension.services(&ctx)[0].name(), "drive-follow");
}

#[test]
fn provide_publishes_the_withheld_handle() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let extension = DriveExtension::default();
    extension.provide(&ctx);
    let handle = ctx.handles.get::<DriveHandle>().unwrap();
    assert!(handle.is_withheld());
    assert_eq!(handle.client().unwrap_err(), DriveWithheld);
    assert!(handle.sync().root().is_none());
}

#[test]
fn the_applier_follows_the_applied_instance_and_clears_it() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let extension = DriveExtension::default();
    let appliers = extension.appliers(&ctx);
    assert_eq!(appliers.len(), 1);
    appliers[0]
        .apply(&applied("https://one.example.test", "account-1"))
        .unwrap();
    let client = extension.handle().client().unwrap();
    assert_eq!(client.origin().as_str(), "https://one.example.test/");
    appliers[0]
        .apply(&applied("https://two.example.test", "account-2"))
        .unwrap();
    let client = extension.handle().client().unwrap();
    assert_eq!(client.origin().as_str(), "https://two.example.test/");
    appliers[0].clear();
    assert!(extension.handle().is_withheld());
    assert_eq!(extension.handle().client().unwrap_err(), DriveWithheld);
}

#[tokio::test]
async fn a_withheld_tool_fails_closed_after_asking() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let extension = DriveExtension::default();
    let tools = extension.engine(&ctx).unwrap().tools();
    let gate = Arc::new(Gate::default());
    let error = tools[0]
        .tool
        .run(json!({ "query": "plan" }), &tool_context(Arc::clone(&gate)))
        .await
        .unwrap_err();
    let ToolError::Failed { message, .. } = error else {
        panic!("expected a failure, got {error:?}");
    };
    assert!(message.contains("cannot be reached"));
    assert_eq!(gate.asked.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn an_applied_tool_asks_the_gate_and_answers_the_instance() {
    let fake = FakeInstance::start(|_| {
        FakeResponse::json(
            200,
            &json!({ "documents": [{
                "id": "00000000-0000-4000-8000-000000000001",
                "name": "quarterly plan",
                "format": "markdown",
                "mime_type": "text/markdown",
                "head_revision_id": REV,
                "owner_user_id": USER,
                "created_at": "t",
                "updated_at": "t"
            }]}),
        )
    })
    .await;
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let extension = DriveExtension::default();
    extension.appliers(&ctx)[0]
        .apply(&applied(&fake.url, "account-1"))
        .unwrap();
    let tools = extension.engine(&ctx).unwrap().tools();
    let gate = Arc::new(Gate::default());
    let output = tools[0]
        .tool
        .run(json!({ "query": "plan" }), &tool_context(Arc::clone(&gate)))
        .await
        .unwrap();
    assert!(output.output.contains("quarterly plan"));
    assert_eq!(fake.last().path(), "/api/v1/documents");
    let asked = gate.asked.lock().unwrap();
    assert_eq!(asked[0].permission, "drive");
    assert_eq!(asked[0].patterns, vec!["drive_search *".to_owned()]);
    assert_eq!(asked[0].always, vec!["drive_search *".to_owned()]);
}

#[tokio::test]
async fn an_input_without_its_id_is_refused_before_asking() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let extension = DriveExtension::default();
    let tools = extension.engine(&ctx).unwrap().tools();
    let read = tools
        .iter()
        .find(|tool| tool.tool.name() == "drive_read")
        .unwrap();
    let gate = Arc::new(Gate::default());
    let error = read
        .tool
        .run(
            json!({ "full_text": true }),
            &tool_context(Arc::clone(&gate)),
        )
        .await;
    assert!(error.is_err());
    assert!(gate.asked.lock().unwrap().is_empty());
}

#[test]
fn document_kinds_name_their_templates() {
    let kind: DocumentKind = serde_json::from_value(json!("word")).unwrap();
    assert_eq!(kind, DocumentKind::Word);
    assert_eq!(kind.template_id(), "blank-docx");
    assert_eq!(DocumentKind::default(), DocumentKind::Markdown);
    assert_eq!(DocumentKind::ALL.len(), 6);
    assert!(serde_json::from_value::<DocumentKind>(json!("pdf")).is_err());
}

#[test]
fn the_folder_setting_is_stored_under_folder() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), Some(dir.path().join("Documents")));
    let settings = ctx.settings_for(&DriveExtension::ID);
    assert_eq!(
        settings.get::<DriveSettings>().unwrap(),
        DriveSettings::default()
    );
    let folder = dir.path().join("picked");
    settings
        .set(&DriveSettings {
            folder: Some(folder.clone()),
        })
        .unwrap();
    let stored = ctx.settings.read(&DriveExtension::ID).unwrap();
    assert_eq!(stored.get("folder"), Some(&json!(folder)));
    assert_eq!(
        DriveExtension::default_folder(&ctx),
        Some(sp_drive::default_drive_root(
            &dir.path().join("Documents"),
            "Drive Tests"
        ))
    );
}
