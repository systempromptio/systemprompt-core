//! Unit tests for resources module (artifact viewer).

use rmcp::model::ReadResourceRequestParams;
use systemprompt_mcp::{
    ArtifactViewerConfig, build_artifact_viewer_resource, default_server_icons,
    read_artifact_viewer_resource,
};

const SAMPLE_TEMPLATE: &str = "<!doctype html><html><body>hello</body></html>";

fn sample_config() -> ArtifactViewerConfig<'static> {
    ArtifactViewerConfig {
        server_name: "demo",
        title: "Demo Viewer",
        description: "A demo artifact viewer",
        template: SAMPLE_TEMPLATE,
        icons: None,
    }
}

#[test]
fn test_build_artifact_viewer_returns_single_resource() {
    let result = build_artifact_viewer_resource(&sample_config());
    assert_eq!(result.resources.len(), 1);
    assert!(result.next_cursor.is_none());
}

#[test]
fn test_build_artifact_viewer_uri_format() {
    let result = build_artifact_viewer_resource(&sample_config());
    let resource = &result.resources[0];
    assert_eq!(resource.uri, "ui://demo/artifact-viewer");
    assert_eq!(resource.name, "Artifact Viewer");
    assert_eq!(resource.title.as_deref(), Some("Demo Viewer"));
    assert_eq!(
        resource.description.as_deref(),
        Some("A demo artifact viewer")
    );
    assert!(resource.mime_type.is_some());
}

#[test]
fn test_build_artifact_viewer_size_matches_template() {
    let result = build_artifact_viewer_resource(&sample_config());
    let resource = &result.resources[0];
    assert_eq!(
        resource.size,
        Some(u64::try_from(SAMPLE_TEMPLATE.len()).expect("fits"))
    );
}

#[test]
fn test_build_artifact_viewer_with_icons() {
    let icons = default_server_icons();
    let config = ArtifactViewerConfig {
        server_name: "demo",
        title: "T",
        description: "D",
        template: "",
        icons: Some(icons.clone()),
    };
    let result = build_artifact_viewer_resource(&config);
    let resource = &result.resources[0];
    assert!(resource.icons.is_some());
    assert_eq!(
        resource.icons.as_ref().expect("icons").len(),
        icons.len()
    );
}

#[test]
fn test_default_server_icons_two_entries() {
    let icons = default_server_icons();
    assert_eq!(icons.len(), 2);
}

#[test]
fn test_default_server_icons_use_website_url() {
    let icons = default_server_icons();
    for icon in &icons {
        assert!(icon.src.contains("systemprompt.io"));
    }
}

#[test]
fn test_read_artifact_viewer_resource_success() {
    let request = ReadResourceRequestParams::new("ui://demo/artifact-viewer");
    let result =
        read_artifact_viewer_resource(&request, "demo", SAMPLE_TEMPLATE).expect("should succeed");
    assert_eq!(result.contents.len(), 1);
}

#[test]
fn test_read_artifact_viewer_resource_wrong_server_name() {
    let request = ReadResourceRequestParams::new("ui://other/artifact-viewer");
    let result = read_artifact_viewer_resource(&request, "demo", SAMPLE_TEMPLATE);
    let err = result.unwrap_err();
    assert!(err.message.contains("Unknown") || err.message.contains("Expected"));
}

#[test]
fn test_read_artifact_viewer_resource_empty_uri() {
    let request = ReadResourceRequestParams::new("");
    let result = read_artifact_viewer_resource(&request, "demo", SAMPLE_TEMPLATE);
    result.unwrap_err();
}

#[test]
fn test_read_artifact_viewer_resource_arbitrary_template_passthrough() {
    let request = ReadResourceRequestParams::new("ui://srv/artifact-viewer");
    let template = "ARBITRARY-PAYLOAD-${VAR}";
    let result = read_artifact_viewer_resource(&request, "srv", template).expect("should succeed");
    // The text contents should contain our template body.
    let serialized = serde_json::to_string(&result.contents).expect("serializable");
    assert!(serialized.contains("ARBITRARY-PAYLOAD"));
}
