use rmcp::model::{
    Icon, ListResourcesResult, Meta, RawResource, ReadResourceRequestParams, ReadResourceResult,
    Resource, ResourceContents,
};
use rmcp::ErrorData as McpError;

use crate::capabilities::WEBSITE_URL;
use crate::services::ui_renderer::{CspPolicy, UiMetadata, MCP_APP_MIME_TYPE};

#[derive(Debug)]
pub struct ArtifactViewerConfig<'a> {
    pub server_name: &'a str,
    pub title: &'a str,
    pub description: &'a str,
    pub template: &'a str,
    pub icons: Option<Vec<Icon>>,
}

#[must_use]
pub fn build_artifact_viewer_resource(config: &ArtifactViewerConfig<'_>) -> ListResourcesResult {
    let resource = Resource {
        raw: RawResource {
            uri: format!("ui://{}/artifact-viewer", config.server_name),
            name: "Artifact Viewer".to_string(),
            title: Some(config.title.to_string()),
            description: Some(config.description.to_string()),
            mime_type: Some(MCP_APP_MIME_TYPE.to_string()),
            size: Some(u32::try_from(config.template.len()).unwrap_or(u32::MAX)),
            icons: config.icons.clone(),
            meta: None,
        },
        annotations: None,
    };

    ListResourcesResult {
        resources: vec![resource],
        next_cursor: None,
        meta: None,
    }
}

pub fn read_artifact_viewer_resource(
    request: &ReadResourceRequestParams,
    server_name: &str,
    template: &str,
) -> Result<ReadResourceResult, McpError> {
    let uri = &request.uri;
    let expected_uri = format!("ui://{server_name}/artifact-viewer");

    if uri != &expected_uri {
        return Err(McpError::invalid_params(
            format!("Unknown resource URI: {uri}. Expected: {expected_uri}"),
            None,
        ));
    }

    let ui_meta = UiMetadata::for_static_template(server_name)
        .with_csp(CspPolicy::strict())
        .with_prefers_border(true);

    let resource_meta = ui_meta.to_resource_meta();
    let meta = Meta(resource_meta.to_meta_map());

    let contents = ResourceContents::TextResourceContents {
        uri: uri.clone(),
        mime_type: Some(MCP_APP_MIME_TYPE.to_string()),
        text: template.to_string(),
        meta: Some(meta),
    };

    Ok(ReadResourceResult::new(vec![contents]))
}

#[must_use]
pub fn default_server_icons() -> Vec<Icon> {
    vec![
        Icon::new(format!("{WEBSITE_URL}/files/images/favicon-32x32.png"))
            .with_mime_type("image/png")
            .with_sizes(vec!["32x32".to_string()]),
        Icon::new(format!("{WEBSITE_URL}/files/images/favicon-96x96.png"))
            .with_mime_type("image/png")
            .with_sizes(vec!["96x96".to_string()]),
    ]
}
