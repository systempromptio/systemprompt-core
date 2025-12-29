use serde_json::Value;
use systemprompt_traits::artifact::{schemas, ArtifactSupport};

struct TestServer;

impl ArtifactSupport for TestServer {
    fn get_output_schema_for_tool(
        &self,
        tool_name: &str,
        _arguments: &serde_json::Map<String, Value>,
    ) -> Option<Value> {
        match tool_name {
            "card_tool" => Some(schemas::presentation_card(Some("gradient"))),
            "table_tool" => Some(schemas::table()),
            _ => None,
        }
    }
}

#[test]
fn test_schema_resolution() {
    let server = TestServer;
    let args = serde_json::Map::new();

    let card_schema = server.get_output_schema_for_tool("card_tool", &args);
    assert!(card_schema.is_some());
    assert_eq!(card_schema.unwrap()["x-artifact-type"], "presentation_card");

    let table_schema = server.get_output_schema_for_tool("table_tool", &args);
    assert!(table_schema.is_some());
    assert_eq!(table_schema.unwrap()["x-artifact-type"], "table");

    let no_schema = server.get_output_schema_for_tool("unknown_tool", &args);
    assert!(no_schema.is_none());
}

#[test]
fn test_validation() {
    let server = TestServer;

    assert!(server.validate_artifact_schema("tool", true, true));
    assert!(server.validate_artifact_schema("tool", false, false));
    assert!(!server.validate_artifact_schema("tool", true, false));
    assert!(server.validate_artifact_schema("tool", false, true));
}

#[test]
fn test_schema_helpers() {
    let card = schemas::presentation_card(Some("dark"));
    assert_eq!(card["x-artifact-type"], "presentation_card");
    assert_eq!(card["x-presentation-hints"]["theme"], "dark");

    let table = schemas::table();
    assert_eq!(table["x-artifact-type"], "table");

    let chart = schemas::chart("bar");
    assert_eq!(chart["x-artifact-type"], "chart");
    assert_eq!(chart["x-chart-type"], "bar");

    let code = schemas::code(Some("rust"));
    assert_eq!(code["x-artifact-type"], "code");

    let markdown = schemas::markdown();
    assert_eq!(markdown["x-artifact-type"], "markdown");
}
