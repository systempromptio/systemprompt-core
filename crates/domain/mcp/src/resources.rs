//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use rmcp::model::{
    Icon, ListResourcesResult, Meta, ReadResourceRequestParams, ReadResourceResult, Resource,
    ResourceContents,
};

use crate::capabilities::WEBSITE_URL;
use crate::services::ui_renderer::{CspPolicy, MCP_APP_MIME_TYPE, UiMetadata};

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
    let mut resource = Resource::new(
        format!("ui://{}/artifact-viewer", config.server_name),
        "Artifact Viewer",
    )
    .with_title(config.title.to_owned())
    .with_description(config.description.to_owned())
    .with_mime_type(MCP_APP_MIME_TYPE.to_owned())
    .with_size(u64::try_from(config.template.len()).unwrap_or(u64::MAX));
    resource.icons.clone_from(&config.icons);

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
        mime_type: Some(MCP_APP_MIME_TYPE.to_owned()),
        text: template.to_owned(),
        meta: Some(meta),
    };

    Ok(ReadResourceResult::new(vec![contents]))
}

#[must_use]
pub fn default_server_icons() -> Vec<Icon> {
    vec![
        Icon::new(format!("{WEBSITE_URL}/files/images/favicon-32x32.png"))
            .with_mime_type("image/png")
            .with_sizes(vec!["32x32".to_owned()]),
        Icon::new(format!("{WEBSITE_URL}/files/images/favicon-96x96.png"))
            .with_mime_type("image/png")
            .with_sizes(vec!["96x96".to_owned()]),
    ]
}
