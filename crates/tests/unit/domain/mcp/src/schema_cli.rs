use systemprompt_mcp::McpOutputSchema;
use systemprompt_models::artifacts::cli::CliArtifact;
use systemprompt_models::artifacts::{
    DashboardArtifact, ListArtifact, PresentationCardArtifact, TableArtifact, TextArtifact,
};

#[test]
fn cli_artifact_type_static_str_is_cli() {
    assert_eq!(CliArtifact::artifact_type(), "cli");
}

#[test]
fn cli_artifact_list_delegates_type_name() {
    let lst = CliArtifact::list(ListArtifact::new());
    assert_eq!(lst.artifact_type_name(), ListArtifact::ARTIFACT_TYPE_STR);
}

#[test]
fn cli_artifact_text_delegates_type_name() {
    let txt = CliArtifact::text(TextArtifact::new("hi"));
    assert_eq!(txt.artifact_type_name(), TextArtifact::ARTIFACT_TYPE_STR);
}

#[test]
fn cli_artifact_dashboard_delegates_type_name() {
    let dash = CliArtifact::dashboard(DashboardArtifact::new("Title"));
    assert_eq!(
        dash.artifact_type_name(),
        DashboardArtifact::ARTIFACT_TYPE_STR
    );
}

#[test]
fn cli_artifact_dashboard_delegates_title() {
    let dash_art = DashboardArtifact::new("My Dashboard");
    let cli = CliArtifact::dashboard(dash_art);
    assert_eq!(cli.artifact_title(), Some("My Dashboard".to_owned()));
}

#[test]
fn cli_artifact_presentation_card_delegates_title() {
    let card = PresentationCardArtifact::new("My Card");
    let cli = CliArtifact::presentation_card(card);
    assert_eq!(cli.artifact_title(), Some("My Card".to_owned()));
}

#[test]
fn cli_artifact_list_no_title() {
    let lst = CliArtifact::list(ListArtifact::new());
    assert!(lst.artifact_title().is_none());
}

#[test]
fn cli_artifact_table_no_title() {
    let tbl = CliArtifact::table(TableArtifact::new(vec![]));
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
    let t = TextArtifact::new("content").with_title("heading");
    assert_eq!(t.artifact_title(), Some("heading".to_owned()));
}

#[test]
fn text_artifact_title_without_value() {
    let t = TextArtifact::new("content");
    assert!(t.artifact_title().is_none());
}

#[test]
fn artifact_type_name_instance_method_matches_static() {
    let t = TextArtifact::new("hi");
    assert_eq!(t.artifact_type_name(), TextArtifact::artifact_type());
}

#[test]
fn dashboard_artifact_title_required() {
    let d = DashboardArtifact::new("Required Title");
    assert_eq!(d.artifact_title(), Some("Required Title".to_owned()));
}

#[test]
fn presentation_card_artifact_title_required() {
    let p = PresentationCardArtifact::new("Required");
    assert_eq!(p.artifact_title(), Some("Required".to_owned()));
}

#[test]
fn table_artifact_no_title() {
    let t = TableArtifact::new(vec![]);
    assert!(t.artifact_title().is_none());
}

#[test]
fn list_artifact_no_title() {
    let l = ListArtifact::new();
    assert!(l.artifact_title().is_none());
}

#[test]
fn cli_table_type_name_delegates() {
    let tbl = CliArtifact::table(TableArtifact::new(vec![]));
    assert_eq!(tbl.artifact_type_name(), TableArtifact::ARTIFACT_TYPE_STR);
}
