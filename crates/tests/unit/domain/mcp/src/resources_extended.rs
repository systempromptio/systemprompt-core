use rmcp::model::ReadResourceRequestParams;
use systemprompt_mcp::{
    ArtifactViewerConfig, WEBSITE_URL, build_artifact_viewer_resource, default_server_icons,
    read_artifact_viewer_resource,
};

#[test]
fn build_artifact_viewer_empty_icons() {
    let config = ArtifactViewerConfig {
        server_name: "no-icons",
        title: "T",
        description: "D",
        template: "HTML",
        icons: Some(vec![]),
    };
    let result = build_artifact_viewer_resource(&config);
    assert_eq!(result.resources.len(), 1);
    let icons = result.resources[0].icons.as_ref().expect("icons");
    assert!(icons.is_empty());
}

#[test]
fn build_artifact_viewer_none_icons() {
    let config = ArtifactViewerConfig {
        server_name: "srv",
        title: "T",
        description: "D",
        template: "X",
        icons: None,
    };
    let result = build_artifact_viewer_resource(&config);
    assert!(result.resources[0].icons.is_none());
}

#[test]
fn build_artifact_viewer_size_zero_for_empty_template() {
    let config = ArtifactViewerConfig {
        server_name: "srv",
        title: "T",
        description: "D",
        template: "",
        icons: None,
    };
    let result = build_artifact_viewer_resource(&config);
    assert_eq!(result.resources[0].size, Some(0));
}

#[test]
fn build_artifact_viewer_large_template_size() {
    let large = "x".repeat(100_000);
    let config = ArtifactViewerConfig {
        server_name: "srv",
        title: "T",
        description: "D",
        template: &large,
        icons: None,
    };
    let result = build_artifact_viewer_resource(&config);
    assert_eq!(result.resources[0].size, Some(100_000));
}

#[test]
fn build_artifact_viewer_uri_scheme_is_ui() {
    let config = ArtifactViewerConfig {
        server_name: "my-srv",
        title: "T",
        description: "D",
        template: "content",
        icons: None,
    };
    let result = build_artifact_viewer_resource(&config);
    let uri = &result.resources[0].uri;
    assert!(uri.starts_with("ui://"));
}

#[test]
fn read_artifact_viewer_content_has_correct_mime() {
    let request = ReadResourceRequestParams::new("ui://svc/artifact-viewer");
    let result = read_artifact_viewer_resource(&request, "svc", "content").expect("ok");
    let serialized = serde_json::to_string(&result.contents).expect("serialize");
    assert!(serialized.contains("mcp-app"));
}

#[test]
fn read_artifact_viewer_wrong_path_errors() {
    let request = ReadResourceRequestParams::new("ui://svc/different-path");
    let result = read_artifact_viewer_resource(&request, "svc", "content");
    assert!(result.is_err());
}

#[test]
fn read_artifact_viewer_unicode_template() {
    let request = ReadResourceRequestParams::new("ui://test/artifact-viewer");
    let template = "<html>こんにちは</html>";
    let result = read_artifact_viewer_resource(&request, "test", template).expect("ok");
    let serialized = serde_json::to_string(&result.contents).expect("serialize");
    assert!(serialized.contains("こんにちは"));
}

#[test]
fn default_server_icons_use_png_mime() {
    let icons = default_server_icons();
    for icon in &icons {
        let mime = icon.mime_type.as_deref().expect("mime type");
        assert_eq!(mime, "image/png");
    }
}

#[test]
fn default_server_icons_have_sizes() {
    let icons = default_server_icons();
    for icon in &icons {
        let sizes = icon.sizes.as_deref().expect("sizes");
        assert_eq!(sizes.len(), 1);
    }
}

#[test]
fn default_server_icons_src_contains_website_url() {
    let icons = default_server_icons();
    for icon in &icons {
        assert!(icon.src.contains(WEBSITE_URL));
    }
}

#[test]
fn default_server_icons_have_32_and_96_sizes() {
    let icons = default_server_icons();
    let all_sizes: Vec<_> = icons
        .iter()
        .filter_map(|i| i.sizes.as_ref())
        .flatten()
        .collect();
    assert!(all_sizes.iter().any(|s| s.contains("32x32")));
    assert!(all_sizes.iter().any(|s| s.contains("96x96")));
}

#[test]
fn build_artifact_viewer_next_cursor_none() {
    let config = ArtifactViewerConfig {
        server_name: "srv",
        title: "T",
        description: "D",
        template: "x",
        icons: None,
    };
    let result = build_artifact_viewer_resource(&config);
    assert!(result.next_cursor.is_none());
}
