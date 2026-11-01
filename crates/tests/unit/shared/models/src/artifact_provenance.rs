use semver::Version;
use systemprompt_identifiers::{
    Actor, AgentName, ContextId, McpExecutionId, SessionId, SkillId, SkillName, TraceId, UserId,
};
use systemprompt_models::artifacts::ExecutionMetadata;
use systemprompt_models::artifacts::metadata::ArtifactProvenance;
use systemprompt_models::bridge::instance::BridgeVersionWindow;
use systemprompt_models::execution::RequestContext;

fn ctx() -> RequestContext {
    RequestContext::new(
        SessionId::new("sess-prov"),
        TraceId::new("trace-prov"),
        ContextId::try_new("00000000-0000-4000-8000-0000000000aa").expect("context"),
        AgentName::try_new("prov-agent").expect("agent"),
        Actor::user(UserId::new("user-prov")),
    )
}

#[test]
fn empty_provenance_has_no_metadata_or_execution() {
    let provenance = ArtifactProvenance::default();
    assert!(provenance.metadata().is_none());
    assert!(provenance.execution_id().is_none());
}

#[test]
fn execution_and_skill_without_a_request_yield_no_metadata() {
    let mut provenance = ArtifactProvenance::default();
    provenance.set_execution_id(McpExecutionId::new("exec-only"));
    provenance.set_skill(SkillId::new("skill-a"), SkillName::new("Skill A"));
    assert_eq!(
        provenance.execution_id().map(McpExecutionId::as_str),
        Some("exec-only")
    );
    assert!(provenance.metadata().is_none());
}

#[test]
fn request_metadata_takes_the_execution_and_skill() {
    let mut provenance = ArtifactProvenance::default();
    provenance.set_request(&ctx());
    provenance.set_execution_id(McpExecutionId::new("exec-1"));
    provenance.set_skill(SkillId::new("skill-b"), SkillName::new("Skill B"));
    let metadata = provenance.metadata().expect("metadata");
    assert_eq!(metadata.trace_id.as_str(), "trace-prov");
    assert_eq!(metadata.agent_name.as_str(), "prov-agent");
    assert_eq!(
        metadata.execution_id.as_ref().map(McpExecutionId::as_str),
        Some("exec-1")
    );
    assert_eq!(
        metadata.skill_id.as_ref().map(SkillId::as_str),
        Some("skill-b")
    );
    assert_eq!(
        metadata.skill_name.as_ref().map(SkillName::as_str),
        Some("Skill B")
    );
}

#[test]
fn the_request_execution_id_is_the_fallback() {
    let mut provenance = ArtifactProvenance::default();
    provenance.set_metadata(
        ExecutionMetadata::with_request(&ctx()).with_execution(McpExecutionId::new("exec-req")),
    );
    assert_eq!(
        provenance.execution_id().map(McpExecutionId::as_str),
        Some("exec-req")
    );
    provenance.set_execution_id(McpExecutionId::new("exec-own"));
    assert_eq!(
        provenance.execution_id().map(McpExecutionId::as_str),
        Some("exec-own")
    );
    let metadata = provenance.metadata().expect("metadata");
    assert_eq!(
        metadata.execution_id.as_ref().map(McpExecutionId::as_str),
        Some("exec-own")
    );
    assert!(metadata.skill_id.is_none());
}

#[test]
fn execution_metadata_schema_and_object_carry_the_fields() {
    let schema = ExecutionMetadata::schema();
    assert!(schema["properties"].get("trace_id").is_some(), "{schema}");
    let object = ExecutionMetadata::with_request(&ctx())
        .to_object()
        .expect("object");
    assert_eq!(object["trace_id"], "trace-prov");
    assert_eq!(object["session_id"], "sess-prov");
}

fn window(min: Option<&str>, max: Option<&str>, enforce: bool) -> BridgeVersionWindow {
    BridgeVersionWindow {
        min: min.map(|v| Version::parse(v).expect("min")),
        max: max.map(|v| Version::parse(v).expect("max")),
        enforce_max: enforce,
    }
}

#[test]
fn intersect_picks_the_other_side_when_it_is_tighter() {
    let loose = window(Some("0.70.0"), Some("0.80.0"), false);
    let tight = window(Some("0.72.0"), Some("0.75.0"), true);
    let both = loose.intersect(&tight);
    assert_eq!(both.min, Some(Version::new(0, 72, 0)));
    assert_eq!(both.max, Some(Version::new(0, 75, 0)));
    assert!(both.enforce_max);
    assert!(!both.is_empty());
}

#[test]
fn intersect_with_one_sided_windows_keeps_each_present_bound() {
    let floor_only = window(Some("0.71.0"), None, false);
    let cap_only = window(None, Some("0.79.0"), false);
    let both = floor_only.intersect(&cap_only);
    assert_eq!(both.min, Some(Version::new(0, 71, 0)));
    assert_eq!(both.max, Some(Version::new(0, 79, 0)));
    assert!(!both.enforce_max);
    let reversed = cap_only.intersect(&floor_only);
    assert_eq!(reversed, both);
    assert!(
        BridgeVersionWindow::default()
            .intersect(&BridgeVersionWindow::default())
            .is_open()
    );
}
