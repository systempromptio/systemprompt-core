//! Server-side rendering of a persisted artifact into an MCP UI resource.
//!
//! Tool results carry their own rendered HTML so the host has nothing to
//! type-dispatch on: [`artifact_ui_resource`] wraps a stored artifact payload
//! in a minimal A2A [`Artifact`] and hands it to the renderer registry, which
//! already knows how to fall through the CLI envelope tag to the concrete
//! variant.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::registry::create_default_registry;
use super::{UiRendererRegistry, UiResource};
use crate::error::{McpDomainError, McpDomainResult};
use serde_json::Value as JsonValue;
use std::sync::{Arc, OnceLock};
use systemprompt_identifiers::{ArtifactId, ContextId, TaskId};
use systemprompt_models::a2a::{Artifact, ArtifactMetadata, DataPart, Part};

#[derive(Debug)]
pub struct RenderTarget<'a> {
    pub artifact_id: &'a ArtifactId,
    pub artifact_type: &'a str,
    pub payload: &'a JsonValue,
    pub context_id: ContextId,
    pub title: Option<String>,
}

fn default_registry() -> Arc<UiRendererRegistry> {
    static REGISTRY: OnceLock<Arc<UiRendererRegistry>> = OnceLock::new();
    Arc::clone(REGISTRY.get_or_init(|| Arc::new(create_default_registry())))
}

pub async fn artifact_ui_resource(target: &RenderTarget<'_>) -> McpDomainResult<UiResource> {
    let artifact = to_a2a_artifact(target)?;
    default_registry().render(&artifact).await
}

pub fn artifact_resource_uri(server_name: &str, artifact_id: &ArtifactId) -> String {
    format!("ui://{server_name}/artifact/{artifact_id}")
}

pub fn parse_artifact_resource_uri(uri: &str) -> Option<(&str, &str)> {
    let rest = uri.strip_prefix("ui://")?;
    let (server_name, tail) = rest.split_once('/')?;
    let artifact_id = tail.strip_prefix("artifact/")?;
    (!server_name.is_empty() && !artifact_id.is_empty()).then_some((server_name, artifact_id))
}

fn to_a2a_artifact(target: &RenderTarget<'_>) -> McpDomainResult<Artifact> {
    let data =
        target.payload.as_object().cloned().ok_or_else(|| {
            McpDomainError::Internal("Artifact payload is not an object".to_owned())
        })?;

    Ok(Artifact {
        id: target.artifact_id.clone(),
        title: target.title.clone(),
        description: None,
        parts: vec![Part::Data(DataPart { data })],
        extensions: Vec::new(),
        metadata: ArtifactMetadata::new(
            target.artifact_type.to_owned(),
            target.context_id.clone(),
            TaskId::generate(),
        ),
    })
}
