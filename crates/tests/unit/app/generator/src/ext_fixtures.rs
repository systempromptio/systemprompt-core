//! Fixtures that make the hook- and asset-driven arms of the generator
//! reachable:
//!
//! - [`fixture_assets`]: a required and an optional (missing) asset declaration
//!   driving the `copy_extension_assets` loop.
//! - [`fixture_hooks`]: the content member's default hooks plus component
//!   renderers (direct, partial-backed, failing, and a duplicate-variable
//!   pair), template-data extenders (ok + failing), content-data providers
//!   (matching, non-matching, failing), a page-data provider, and page
//!   prerenderers (renderable, duplicate page type, and one returning `None`),
//!   built through `ContentSlot` as the runtime builds it.
//!
//! The fixtures are deliberately side-effect-free for templates that do not
//! reference their variables.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_content::{
    ContentHooks, ContentSlot, ContentSlotBuilder, default_branding_provider,
    default_homepage_prerenderer, default_list_branding_provider, default_list_items_renderer,
};
use systemprompt_database::DbPool;
use systemprompt_extension::{AssetDefinition, AssetPaths, AssetType, Resolved, Slot, SlotEntry};
use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::{
    ComponentContext, ComponentRenderer, ContentDataContext, ContentDataProvider, Dependencies,
    ExtenderContext, PageContext, PageDataProvider, PagePrepareContext, PagePrerenderer,
    PageRenderSpec, PartialTemplate, ProviderError, ProviderResult, RenderedComponent,
    TemplateDataExtender,
};
use systemprompt_runtime::Kernel;
use systemprompt_traits::{BackgroundTasks, DatabaseHandle};

pub const GEN_REQUIRED_ASSET_SOURCE: &str = "genassets_ok/present.css";
pub const GEN_REQUIRED_ASSET_DEST: &str = "css/genassets-ok.css";

pub fn fixture_assets(paths: &dyn AssetPaths) -> Vec<(ExtensionId, AssetDefinition)> {
    vec![
        (
            ExtensionId::new("genassets_ok"),
            AssetDefinition::css(
                paths.storage_files().join(GEN_REQUIRED_ASSET_SOURCE),
                GEN_REQUIRED_ASSET_DEST,
            ),
        ),
        (
            ExtensionId::new("genassets_optmissing"),
            AssetDefinition::builder(
                paths
                    .storage_files()
                    .join("genassets_optmissing/absent.css"),
                "css/genassets-absent.css",
                AssetType::Css,
            )
            .optional()
            .build(),
        ),
    ]
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
            return Err(ProviderError::Internal("component fixture boom".into()));
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
            return Err(ProviderError::Internal("extender fixture boom".into()));
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
            return Err(ProviderError::Internal("content data fixture boom".into()));
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

pub fn fixture_hooks(db: &DbPool) -> ContentHooks {
    let mut builder = ContentSlotBuilder::default();
    builder
        .page_prerenderer(|_: &Resolved<'_>| Ok(default_homepage_prerenderer()))
        .page_data_provider(|_: &Resolved<'_>| Ok(default_branding_provider()))
        .page_data_provider(|_: &Resolved<'_>| Ok(default_list_branding_provider()))
        .component_renderer(|_: &Resolved<'_>| Ok(default_list_items_renderer()));
    builder.component_renderer(|_: &Resolved<'_>| {
        Ok(Arc::new(GenComponent {
            id: "gen_comp_hi",
            variable: "GEN_COMPONENT",
            priority: 10,
            fails: false,
            partial: false,
        }) as Arc<dyn ComponentRenderer>)
    });
    builder.component_renderer(|_: &Resolved<'_>| {
        Ok(Arc::new(GenComponent {
            id: "gen_comp_lo",
            variable: "GEN_COMPONENT",
            priority: 200,
            fails: false,
            partial: false,
        }) as Arc<dyn ComponentRenderer>)
    });
    builder.component_renderer(|_: &Resolved<'_>| {
        Ok(Arc::new(GenComponent {
            id: "gen_comp_partial",
            variable: "GEN_PARTIAL",
            priority: 20,
            fails: false,
            partial: true,
        }) as Arc<dyn ComponentRenderer>)
    });
    builder.component_renderer(|_: &Resolved<'_>| {
        Ok(Arc::new(GenComponent {
            id: "gen_comp_fail",
            variable: "GEN_FAILING",
            priority: 30,
            fails: true,
            partial: false,
        }) as Arc<dyn ComponentRenderer>)
    });
    builder.template_data_extender(|_: &Resolved<'_>| {
        Ok(Arc::new(GenExtender {
            id: "gen_ext_ok",
            fails: false,
        }) as Arc<dyn TemplateDataExtender>)
    });
    builder.template_data_extender(|_: &Resolved<'_>| {
        Ok(Arc::new(GenExtender {
            id: "gen_ext_fail",
            fails: true,
        }) as Arc<dyn TemplateDataExtender>)
    });
    builder.page_data_provider(|_: &Resolved<'_>| {
        Ok(Arc::new(GenPageData) as Arc<dyn PageDataProvider>)
    });
    builder.page_prerenderer(|_: &Resolved<'_>| {
        Ok(Arc::new(GenPrerenderer {
            page_type: "covgenpage",
            priority: 10,
            returns_none: false,
        }) as Arc<dyn PagePrerenderer>)
    });
    builder.page_prerenderer(|_: &Resolved<'_>| {
        Ok(Arc::new(GenPrerenderer {
            page_type: "covgenpage",
            priority: 200,
            returns_none: false,
        }) as Arc<dyn PagePrerenderer>)
    });
    builder.page_prerenderer(|_: &Resolved<'_>| {
        Ok(Arc::new(GenPrerenderer {
            page_type: "covgennone",
            priority: 100,
            returns_none: true,
        }) as Arc<dyn PagePrerenderer>)
    });
    builder.content_data_provider(|_: &Resolved<'_>| {
        Ok(Arc::new(GenContentData {
            id: "gen_cd_all",
            sources: vec!["blog".to_owned()],
            fails: false,
        }) as Arc<dyn ContentDataProvider>)
    });
    builder.content_data_provider(|_: &Resolved<'_>| {
        Ok(Arc::new(GenContentData {
            id: "gen_cd_never",
            sources: vec!["never_matches_source".to_owned()],
            fails: false,
        }) as Arc<dyn ContentDataProvider>)
    });
    builder.content_data_provider(|_: &Resolved<'_>| {
        Ok(Arc::new(GenContentData {
            id: "gen_cd_fail",
            sources: vec![],
            fails: true,
        }) as Arc<dyn ContentDataProvider>)
    });
    let handle: Arc<dyn DatabaseHandle> = Arc::<systemprompt_database::Database>::clone(db);
    let kernel = Kernel::new(handle, BackgroundTasks::new(), Dependencies::default());
    ContentSlot::build(
        vec![SlotEntry::install(
            ExtensionId::new("genproviders"),
            builder,
        )],
        &kernel.resolved(),
    )
    .expect("the fixture hooks build")
}
