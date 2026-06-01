use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId};
use systemprompt_mcp::McpOutputSchema;
use systemprompt_models::RequestContext;
use systemprompt_models::artifacts::cli::CliArtifact;
use systemprompt_models::artifacts::{
    DashboardArtifact, ListArtifact, PresentationCardArtifact, TableArtifact, TextArtifact,
};

fn ctx() -> RequestContext {
    RequestContext::new(
        SessionId::new("s"),
        TraceId::new("t"),
        ContextId::new("00000000-0000-4000-8000-000000000001"),
        AgentName::new("a"),
    )
}

#[test]
fn cli_artifact_type_static_str_is_cli() {
    assert_eq!(CliArtifact::artifact_type(), "cli");
}

#[test]
fn cli_artifact_list_delegates_type_name() {
    let c = ctx();
    let lst = CliArtifact::list(ListArtifact::new(&c));
    assert_eq!(lst.artifact_type_name(), ListArtifact::ARTIFACT_TYPE_STR);
}

#[test]
fn cli_artifact_text_delegates_type_name() {
    let c = ctx();
    let txt = CliArtifact::text(TextArtifact::new("hi", &c));
    assert_eq!(txt.artifact_type_name(), TextArtifact::ARTIFACT_TYPE_STR);
}

#[test]
fn cli_artifact_dashboard_delegates_type_name() {
    let c = ctx();
    let dash = CliArtifact::dashboard(DashboardArtifact::new("Title", &c));
    assert_eq!(
        dash.artifact_type_name(),
        DashboardArtifact::ARTIFACT_TYPE_STR
    );
}

#[test]
fn cli_artifact_dashboard_delegates_title() {
    let c = ctx();
    let dash_art = DashboardArtifact::new("My Dashboard", &c);
    let cli = CliArtifact::dashboard(dash_art);
    assert_eq!(cli.artifact_title(), Some("My Dashboard".to_owned()));
}

#[test]
fn cli_artifact_presentation_card_delegates_title() {
    let c = ctx();
    let card = PresentationCardArtifact::new("My Card", &c);
    let cli = CliArtifact::presentation_card(card);
    assert_eq!(cli.artifact_title(), Some("My Card".to_owned()));
}

#[test]
fn cli_artifact_list_no_title() {
    let c = ctx();
    let lst = CliArtifact::list(ListArtifact::new(&c));
    assert!(lst.artifact_title().is_none());
}

#[test]
fn cli_artifact_table_no_title() {
    let c = ctx();
    let tbl = CliArtifact::table(TableArtifact::new(vec![], &c));
    assert!(tbl.artifact_title().is_none());
}

#[test]
fn cli_artifact_validated_schema_is_object() {
    let schema = <CliArtifact as McpOutputSchema>::validated_schema();
    assert!(schema.is_object());
}

#[test]
fn cli_artifact_validated_schema_has_x_artifact_type_tag() {
    let schema = <CliArtifact as McpOutputSchema>::validated_schema();
    let tag = schema
        .get("x-artifact-type")
        .and_then(|v| v.as_str())
        .expect("x-artifact-type");
    assert_eq!(tag, "cli");
}

#[test]
fn text_artifact_title_with_value() {
    let c = ctx();
    let t = TextArtifact::new("content", &c).with_title("heading");
    assert_eq!(t.artifact_title(), Some("heading".to_owned()));
}

#[test]
fn text_artifact_title_without_value() {
    let c = ctx();
    let t = TextArtifact::new("content", &c);
    assert!(t.artifact_title().is_none());
}

#[test]
fn artifact_type_name_instance_method_matches_static() {
    let c = ctx();
    let t = TextArtifact::new("hi", &c);
    assert_eq!(t.artifact_type_name(), TextArtifact::artifact_type());
}

#[test]
fn dashboard_artifact_title_required() {
    let c = ctx();
    let d = DashboardArtifact::new("Required Title", &c);
    assert_eq!(d.artifact_title(), Some("Required Title".to_owned()));
}

#[test]
fn presentation_card_artifact_title_required() {
    let c = ctx();
    let p = PresentationCardArtifact::new("Required", &c);
    assert_eq!(p.artifact_title(), Some("Required".to_owned()));
}

#[test]
fn table_artifact_no_title() {
    let c = ctx();
    let t = TableArtifact::new(vec![], &c);
    assert!(t.artifact_title().is_none());
}

#[test]
fn list_artifact_no_title() {
    let c = ctx();
    let l = ListArtifact::new(&c);
    assert!(l.artifact_title().is_none());
}

#[test]
fn cli_table_type_name_delegates() {
    let c = ctx();
    let tbl = CliArtifact::table(TableArtifact::new(vec![], &c));
    assert_eq!(tbl.artifact_type_name(), TableArtifact::ARTIFACT_TYPE_STR);
}
