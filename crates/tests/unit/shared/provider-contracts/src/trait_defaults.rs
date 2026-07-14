//! Default-method contracts of the provider traits.
//!
//! Every provider trait ships defaults (`applies_to_sources` → all sources,
//! `priority` → 100, `find_tool` → filter over `list_tools`). Extensions rely
//! on these without overriding them, so the defaults are public contract.

use std::collections::HashMap;

use async_trait::async_trait;
use serde_json::Value;
use systemprompt_provider_contracts::{
    ContentDataContext, ContentDataProvider, FrontmatterContext, FrontmatterProcessor, PathsConfig,
    ProviderResult, RssFeedContext, RssFeedItem, RssFeedMetadata, RssFeedProvider, RssFeedSpec,
    TemplateDefinition, TemplateProvider, ToolContext, ToolDefinition, ToolProvider,
    ToolProviderResult,
};

struct MinimalContentData;

#[async_trait]
impl ContentDataProvider for MinimalContentData {
    fn provider_id(&self) -> &'static str {
        "minimal"
    }

    async fn enrich_content(
        &self,
        _ctx: &ContentDataContext<'_>,
        _item: &mut Value,
    ) -> ProviderResult<()> {
        Ok(())
    }
}

struct MinimalFrontmatter;

#[async_trait]
impl FrontmatterProcessor for MinimalFrontmatter {
    fn processor_id(&self) -> &'static str {
        "minimal"
    }

    async fn process_frontmatter(&self, _ctx: &FrontmatterContext<'_>) -> ProviderResult<()> {
        Ok(())
    }
}

struct MinimalRss;

#[async_trait]
impl RssFeedProvider for MinimalRss {
    fn provider_id(&self) -> &'static str {
        "minimal"
    }

    fn feed_specs(&self) -> Vec<RssFeedSpec> {
        vec![]
    }

    async fn feed_metadata(&self, _ctx: &RssFeedContext<'_>) -> ProviderResult<RssFeedMetadata> {
        Ok(RssFeedMetadata {
            title: String::new(),
            link: String::new(),
            description: String::new(),
            language: None,
        })
    }

    async fn fetch_items(
        &self,
        _ctx: &RssFeedContext<'_>,
        _limit: i64,
    ) -> ProviderResult<Vec<RssFeedItem>> {
        Ok(vec![])
    }
}

struct MinimalTemplates;

impl TemplateProvider for MinimalTemplates {
    fn templates(&self) -> Vec<TemplateDefinition> {
        vec![]
    }

    fn provider_id(&self) -> &'static str {
        "minimal"
    }
}

#[test]
fn unoverridden_providers_apply_to_all_sources_at_default_priority() {
    assert!(MinimalContentData.applies_to_sources().is_empty());
    assert_eq!(MinimalContentData.priority(), 100);
    assert!(MinimalFrontmatter.applies_to_sources().is_empty());
    assert_eq!(MinimalFrontmatter.priority(), 100);
    assert_eq!(MinimalRss.priority(), 100);
    assert_eq!(MinimalTemplates.priority(), 100);
}

struct TwoToolProvider;

#[async_trait]
impl ToolProvider for TwoToolProvider {
    async fn list_tools(
        &self,
        _agent_name: &str,
        _context: &ToolContext,
    ) -> ToolProviderResult<Vec<ToolDefinition>> {
        Ok(vec![
            ToolDefinition::new("alpha", "svc"),
            ToolDefinition::new("beta", "svc"),
        ])
    }

    async fn refresh_connections(&self, _agent_name: &str) -> ToolProviderResult<()> {
        Ok(())
    }

    async fn health_check(&self) -> ToolProviderResult<HashMap<String, bool>> {
        Ok(HashMap::new())
    }

    async fn call_tool(
        &self,
        _request: &systemprompt_provider_contracts::ToolCallRequest,
        _server: &systemprompt_identifiers::McpServerId,
        _context: &ToolContext,
    ) -> Result<
        systemprompt_provider_contracts::ToolCallResult,
        systemprompt_provider_contracts::ToolProviderError,
    > {
        unimplemented!("not exercised by these tests")
    }
}

#[tokio::test]
async fn default_find_tool_selects_by_name_from_list_tools() {
    let ctx = ToolContext::new(
        systemprompt_identifiers::Actor::system(systemprompt_identifiers::UserId::new("system")),
        "token",
    );

    let found = TwoToolProvider
        .find_tool("agent", "beta", &ctx)
        .await
        .expect("find_tool succeeds");
    assert_eq!(found.expect("beta exists").name, "beta");

    let missing = TwoToolProvider
        .find_tool("agent", "gamma", &ctx)
        .await
        .expect("find_tool succeeds");
    assert!(missing.is_none());
}

fn paths(templates: &str) -> PathsConfig {
    PathsConfig {
        templates: templates.to_owned(),
        assets: "assets".to_owned(),
        css_url_prefix: None,
    }
}

#[test]
fn relative_template_paths_resolve_against_the_base_directory() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    std::fs::create_dir(dir.path().join("templates")).expect("mkdir");

    let mut existing = paths("templates");
    existing.resolve_relative_to(dir.path());
    let canonical = dir
        .path()
        .join("templates")
        .canonicalize()
        .expect("canonicalize");
    assert_eq!(existing.templates, canonical.to_string_lossy());

    let mut missing = paths("does-not-exist");
    missing.resolve_relative_to(dir.path());
    assert_eq!(
        missing.templates,
        dir.path().join("does-not-exist").to_string_lossy()
    );
}

#[test]
fn absolute_and_empty_template_paths_pass_through_unchanged() {
    let mut absolute = paths("/opt/templates");
    absolute.resolve_relative_to(std::path::Path::new("/base"));
    assert_eq!(absolute.templates, "/opt/templates");

    let mut empty = paths("");
    empty.resolve_relative_to(std::path::Path::new("/base"));
    assert_eq!(empty.templates, "");
}
