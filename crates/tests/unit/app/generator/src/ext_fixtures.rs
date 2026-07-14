//! Inventory-registered fixture extensions that make the extension-driven
//! arms of the generator reachable in this test binary:
//!
//! - `genassets_ok` / `genassets_optmissing`: required + optional asset
//!   declarations driving the `copy_extension_assets` loop.
//! - `genproviders`: component renderers (direct, partial-backed, failing, and
//!   a duplicate-variable pair), template-data extenders (ok + failing),
//!   content-data providers (matching, non-matching, failing), a page-data
//!   provider, and page prerenderers (renderable, duplicate page type, and one
//!   returning `None`).
//!
//! Registration is per-binary (inventory), so every test in this crate that
//! reaches extension discovery sees these; the fixtures are deliberately
//! side-effect-free for templates that do not reference their variables.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::{
    AssetDefinition, AssetPaths, AssetType, Extension, ExtensionMetadata, register_extension,
};
use systemprompt_provider_contracts::{
    ComponentContext, ComponentRenderer, ContentDataContext, ContentDataProvider, ExtenderContext,
    PageContext, PageDataProvider, PagePrepareContext, PagePrerenderer, PageRenderSpec,
    PartialTemplate, ProviderError, ProviderResult, RenderedComponent, TemplateDataExtender,
};

pub const GEN_REQUIRED_ASSET_SOURCE: &str = "genassets_ok/present.css";
pub const GEN_REQUIRED_ASSET_DEST: &str = "css/genassets-ok.css";

#[derive(Default)]
struct GenAssetsOk;

impl Extension for GenAssetsOk {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "genassets_ok",
            name: "Generator Required Asset Fixture",
            version: "0.0.1",
        }
    }

    fn declares_assets(&self) -> bool {
        true
    }

    fn required_assets(&self, paths: &dyn AssetPaths) -> Vec<AssetDefinition> {
        vec![AssetDefinition::css(
            paths.storage_files().join(GEN_REQUIRED_ASSET_SOURCE),
            GEN_REQUIRED_ASSET_DEST,
        )]
    }
}

#[derive(Default)]
struct GenAssetsOptMissing;

impl Extension for GenAssetsOptMissing {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "genassets_optmissing",
            name: "Generator Optional Missing Asset Fixture",
            version: "0.0.1",
        }
    }

    fn declares_assets(&self) -> bool {
        true
    }

    fn required_assets(&self, paths: &dyn AssetPaths) -> Vec<AssetDefinition> {
        vec![
            AssetDefinition::builder(
                paths
                    .storage_files()
                    .join("genassets_optmissing/absent.css"),
                "css/genassets-absent.css",
                AssetType::Css,
            )
            .optional()
            .build(),
        ]
    }
}

struct GenComponent {
    id: &'static str,
    variable: &'static str,
    priority: u32,
    fails: bool,
    partial: bool,
}

#[async_trait]
impl ComponentRenderer for GenComponent {
    fn component_id(&self) -> &'static str {
        self.id
    }

    fn variable_name(&self) -> &'static str {
        self.variable
    }

    fn applies_to(&self) -> Vec<String> {
        vec!["article".to_owned()]
    }

    fn partial_template(&self) -> Option<PartialTemplate> {
        self.partial
            .then(|| PartialTemplate::embedded("gen_fixture_partial", "<i>partial:{{SLUG}}</i>"))
    }

    fn priority(&self) -> u32 {
        self.priority
    }

    async fn render(&self, _ctx: &ComponentContext<'_>) -> ProviderResult<RenderedComponent> {
        if self.fails {
            return Err(ProviderError::RenderFailed("component fixture boom".into()));
        }
        Ok(RenderedComponent::new(
            self.variable,
            format!("<b>component:{}</b>", self.id),
        ))
    }
}

struct GenExtender {
    id: &'static str,
    fails: bool,
}

#[async_trait]
impl TemplateDataExtender for GenExtender {
    fn extender_id(&self) -> &str {
        self.id
    }

    fn applies_to(&self) -> Vec<String> {
        vec!["article".to_owned()]
    }

    async fn extend(
        &self,
        ctx: &ExtenderContext<'_>,
        data: &mut serde_json::Value,
    ) -> ProviderResult<()> {
        if self.fails {
            return Err(ProviderError::RenderFailed("extender fixture boom".into()));
        }
        let enriched = ctx
            .item
            .get("GEN_ENRICHED")
            .and_then(|v| v.as_str())
            .unwrap_or("unenriched")
            .to_owned();
        if let Some(obj) = data.as_object_mut() {
            obj.insert(
                "GEN_EXTENDED".to_owned(),
                serde_json::Value::String("extended".to_owned()),
            );
            obj.insert(
                "GEN_ENRICHED".to_owned(),
                serde_json::Value::String(enriched),
            );
        }
        Ok(())
    }
}

struct GenContentData {
    id: &'static str,
    sources: Vec<String>,
    fails: bool,
}

#[async_trait]
impl ContentDataProvider for GenContentData {
    fn provider_id(&self) -> &'static str {
        self.id
    }

    fn applies_to_sources(&self) -> Vec<String> {
        self.sources.clone()
    }

    async fn enrich_content(
        &self,
        _ctx: &ContentDataContext<'_>,
        item: &mut serde_json::Value,
    ) -> ProviderResult<()> {
        if self.fails {
            return Err(ProviderError::RenderFailed(
                "content data fixture boom".into(),
            ));
        }
        if let Some(obj) = item.as_object_mut() {
            obj.insert(
                "GEN_ENRICHED".to_owned(),
                serde_json::Value::String(self.id.to_owned()),
            );
        }
        Ok(())
    }
}

struct GenPageData;

#[async_trait]
impl PageDataProvider for GenPageData {
    fn provider_id(&self) -> &'static str {
        "gen_page_data"
    }

    fn applies_to_pages(&self) -> Vec<String> {
        vec!["covgenpage".to_owned()]
    }

    async fn provide_page_data(&self, _ctx: &PageContext<'_>) -> ProviderResult<serde_json::Value> {
        Ok(serde_json::json!({ "GEN_PAGE_FIELD": "from-provider" }))
    }
}

struct GenPrerenderer {
    page_type: &'static str,
    priority: u32,
    returns_none: bool,
}

#[async_trait]
impl PagePrerenderer for GenPrerenderer {
    fn page_type(&self) -> &str {
        self.page_type
    }

    fn priority(&self) -> u32 {
        self.priority
    }

    async fn prepare(
        &self,
        _ctx: &PagePrepareContext<'_>,
    ) -> ProviderResult<Option<PageRenderSpec>> {
        if self.returns_none {
            return Ok(None);
        }
        Ok(Some(PageRenderSpec {
            template_name: "covgenpage".to_owned(),
            output_path: "covgen/index.html".into(),
            base_data: serde_json::json!({ "GEN_BASE": "base" }),
        }))
    }
}

#[derive(Default)]
struct GenProviders;

impl Extension for GenProviders {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "genproviders",
            name: "Generator Provider Fixture",
            version: "0.0.1",
        }
    }

    fn component_renderers(&self) -> Vec<Arc<dyn ComponentRenderer>> {
        vec![
            Arc::new(GenComponent {
                id: "gen_comp_hi",
                variable: "GEN_COMPONENT",
                priority: 10,
                fails: false,
                partial: false,
            }),
            Arc::new(GenComponent {
                id: "gen_comp_lo",
                variable: "GEN_COMPONENT",
                priority: 200,
                fails: false,
                partial: false,
            }),
            Arc::new(GenComponent {
                id: "gen_comp_partial",
                variable: "GEN_PARTIAL",
                priority: 20,
                fails: false,
                partial: true,
            }),
            Arc::new(GenComponent {
                id: "gen_comp_fail",
                variable: "GEN_FAILING",
                priority: 30,
                fails: true,
                partial: false,
            }),
        ]
    }

    fn template_data_extenders(&self) -> Vec<Arc<dyn TemplateDataExtender>> {
        vec![
            Arc::new(GenExtender {
                id: "gen_ext_ok",
                fails: false,
            }),
            Arc::new(GenExtender {
                id: "gen_ext_fail",
                fails: true,
            }),
        ]
    }

    fn page_data_providers(&self) -> Vec<Arc<dyn PageDataProvider>> {
        vec![Arc::new(GenPageData)]
    }

    fn page_prerenderers(&self) -> Vec<Arc<dyn PagePrerenderer>> {
        vec![
            Arc::new(GenPrerenderer {
                page_type: "covgenpage",
                priority: 10,
                returns_none: false,
            }),
            Arc::new(GenPrerenderer {
                page_type: "covgenpage",
                priority: 200,
                returns_none: false,
            }),
            Arc::new(GenPrerenderer {
                page_type: "covgennone",
                priority: 100,
                returns_none: true,
            }),
        ]
    }

    fn content_data_providers(&self) -> Vec<Arc<dyn ContentDataProvider>> {
        vec![
            Arc::new(GenContentData {
                id: "gen_cd_all",
                sources: vec!["blog".to_owned()],
                fails: false,
            }),
            Arc::new(GenContentData {
                id: "gen_cd_never",
                sources: vec!["never_matches_source".to_owned()],
                fails: false,
            }),
            Arc::new(GenContentData {
                id: "gen_cd_fail",
                sources: vec![],
                fails: true,
            }),
        ]
    }
}

register_extension!(GenAssetsOk);
register_extension!(GenAssetsOptMissing);
register_extension!(GenProviders);
