use serde_json::json;
use systemprompt_identifiers::{ContextId, SkillId, TaskId};
use systemprompt_models::a2a::artifact_metadata::ArtifactMetadata;
use systemprompt_traits::validation::Validate;

fn ctx() -> ContextId {
    ContextId::new("00000000-0000-4000-8000-000000000001")
}

fn task() -> TaskId {
    TaskId::new("task-1")
}

#[test]
fn new_sets_required_fields_and_defaults() {
    let m = ArtifactMetadata::new("text".to_owned(), ctx(), task());
    assert_eq!(m.artifact_type, "text");
    assert_eq!(m.context_id, ctx());
    assert_eq!(m.task_id, task());
    assert!(m.created_at.contains('T'));
    assert_eq!(m.source.as_deref(), Some("mcp_tool"));
    assert!(m.rendering_hints.is_none());
    assert!(m.mcp_execution_id.is_none());
    assert!(m.skill_id.is_none());
    assert!(m.skill_name.is_none());
}

#[test]
fn builders_set_each_optional_field() {
    let m = ArtifactMetadata::new("text".to_owned(), ctx(), task())
        .with_rendering_hints(json!({"a": 1}))
        .with_source("plugin".to_owned())
        .with_mcp_execution_id("exec".to_owned())
        .with_mcp_schema(json!({"type": "object"}))
        .with_is_internal(true)
        .with_fingerprint("fp".to_owned())
        .with_tool_name("tool".to_owned())
        .with_execution_index(7);

    assert_eq!(m.rendering_hints, Some(json!({"a": 1})));
    assert_eq!(m.source.as_deref(), Some("plugin"));
    assert_eq!(m.mcp_execution_id.as_deref(), Some("exec"));
    assert_eq!(m.mcp_schema, Some(json!({"type": "object"})));
    assert_eq!(m.is_internal, Some(true));
    assert_eq!(m.fingerprint.as_deref(), Some("fp"));
    assert_eq!(m.tool_name.as_deref(), Some("tool"));
    assert_eq!(m.execution_index, Some(7));
}

#[test]
fn with_skill_id_alone_sets_only_id() {
    let skill = SkillId::new("skill-x");
    let m = ArtifactMetadata::new("t".to_owned(), ctx(), task()).with_skill_id(skill.clone());
    assert_eq!(m.skill_id, Some(skill));
    assert!(m.skill_name.is_none());
}

#[test]
fn with_skill_name_alone_sets_only_name() {
    let m =
        ArtifactMetadata::new("t".to_owned(), ctx(), task()).with_skill_name("display".to_owned());
    assert_eq!(m.skill_name.as_deref(), Some("display"));
    assert!(m.skill_id.is_none());
}

#[test]
fn with_skill_sets_both_fields() {
    let skill = SkillId::new("skill-y");
    let m = ArtifactMetadata::new("t".to_owned(), ctx(), task())
        .with_skill(skill.clone(), "Display".to_owned());
    assert_eq!(m.skill_id, Some(skill));
    assert_eq!(m.skill_name.as_deref(), Some("Display"));
}

#[test]
fn validate_succeeds_when_required_fields_present() {
    let m = ArtifactMetadata::new("text".to_owned(), ctx(), task());
    assert!(m.validate().is_ok());
}

#[test]
fn validate_fails_for_empty_artifact_type() {
    let m = ArtifactMetadata::new(String::new(), ctx(), task());
    assert!(m.validate().is_err());
}

#[test]
fn new_validated_rejects_empty_artifact_type() {
    let err = ArtifactMetadata::new_validated(String::new(), ctx(), task()).unwrap_err();
    assert_eq!(err.field, "artifact_type");
}

#[test]
fn new_validated_succeeds_when_valid() {
    let m = ArtifactMetadata::new_validated("ok".to_owned(), ctx(), task()).unwrap();
    assert_eq!(m.artifact_type, "ok");
}

#[test]
fn artifact_metadata_serde_round_trip() {
    let m = ArtifactMetadata::new("text".to_owned(), ctx(), task())
        .with_fingerprint("fp".to_owned())
        .with_is_internal(false);
    let json = serde_json::to_value(&m).unwrap();
    assert!(json.get("rendering_hints").is_none());
    assert!(json.get("tool_name").is_none());
    let back: ArtifactMetadata = serde_json::from_value(json).unwrap();
    assert_eq!(back, m);
}
