//! Behavioral tests for the default page-data providers and prerenderer.
//!
//! Each provider resolves its `ContentConfigRaw` out of the type-erased
//! `PageContext`/`PagePrepareContext`; the tests exercise both the successful
//! projection and the configuration-error branch that fires when the erased
//! value is not a `ContentConfigRaw`.

use std::collections::HashMap;
use systemprompt_content::{
    DefaultBrandingProvider, DefaultHomepagePrerenderer, DefaultListBrandingProvider,
};
use systemprompt_models::content_config::{
    ContentConfigRaw, ContentSourceConfigRaw, Metadata, OrganizationData, SourceBranding,
    StructuredData,
};
use systemprompt_provider_contracts::{
    PageContext, PageDataProvider, PagePrepareContext, PagePrerenderer, ProviderError,
};
use systemprompt_test_fixtures::web_config;

fn config_with_org() -> ContentConfigRaw {
    ContentConfigRaw {
        metadata: Metadata {
            default_author: String::new(),
            structured_data: StructuredData {
                organization: OrganizationData {
                    name: "Acme Docs".to_owned(),
                    url: "https://acme.example".to_owned(),
                    logo: "https://acme.example/logo.svg".to_owned(),
                },
                ..StructuredData::default()
            },
        },
        ..ContentConfigRaw::default()
    }
}

fn source_with_branding(branding: Option<SourceBranding>) -> ContentSourceConfigRaw {
    ContentSourceConfigRaw {
        path: "content/blog".to_owned(),
        source_id: systemprompt_identifiers::SourceId::new("blog"),
        category_id: systemprompt_identifiers::CategoryId::new("tech"),
        enabled: true,
        description: String::new(),
        allowed_content_types: vec![],
        indexing: None,
        sitemap: None,
        branding,
    }
}

#[tokio::test]
async fn branding_provider_projects_org_and_branding_fields() {
    let wc = web_config();
    let cfg = config_with_org();
    let ctx = PageContext::new("homepage", &wc, &cfg, &());

    let data = DefaultBrandingProvider
        .provide_page_data(&ctx)
        .await
        .expect("branding data");

    assert_eq!(data["ORG_NAME"], "Acme Docs");
    assert_eq!(data["ORG_URL"], "https://acme.example");
    assert_eq!(data["ORG_LOGO"], "https://acme.example/logo.svg");
    assert_eq!(data["DISPLAY_SITENAME"], wc.branding.display_sitename);
}

#[tokio::test]
async fn branding_provider_errors_when_content_config_absent() {
    let wc = web_config();
    let not_a_config = 42_u32;
    let ctx = PageContext::new("homepage", &wc, &not_a_config, &());

    let err = DefaultBrandingProvider
        .provide_page_data(&ctx)
        .await
        .expect_err("missing ContentConfig must error");

    assert!(matches!(err, ProviderError::Configuration(_)));
}

#[tokio::test]
async fn list_branding_provider_returns_empty_for_non_list_page() {
    let wc = web_config();
    let cfg = config_with_org();
    let ctx = PageContext::new("homepage", &wc, &cfg, &());

    let data = DefaultListBrandingProvider
        .provide_page_data(&ctx)
        .await
        .expect("empty object");

    assert_eq!(data, serde_json::json!({}));
}

#[tokio::test]
async fn list_branding_provider_errors_when_content_config_absent() {
    let wc = web_config();
    let not_a_config = "nope";
    let ctx = PageContext::new("blog-list", &wc, &not_a_config, &());

    let err = DefaultListBrandingProvider
        .provide_page_data(&ctx)
        .await
        .expect_err("list page without ContentConfig must error");

    assert!(matches!(err, ProviderError::Configuration(_)));
}

#[tokio::test]
async fn list_branding_provider_prefers_source_branding_over_defaults() {
    let wc = web_config();
    let mut cfg = config_with_org();
    let mut sources = HashMap::new();
    sources.insert(
        "blog".to_owned(),
        source_with_branding(Some(SourceBranding {
            name: Some("Engineering Blog".to_owned()),
            description: Some("Deep dives".to_owned()),
            image: Some("/img/blog.png".to_owned()),
            keywords: Some("rust,systems".to_owned()),
        })),
    );
    cfg.content_sources = sources;

    let ctx = PageContext::new("blog-list", &wc, &cfg, &());
    let data = DefaultListBrandingProvider
        .provide_page_data(&ctx)
        .await
        .expect("branding data");

    assert_eq!(data["BLOG_NAME"], "Engineering Blog");
    assert_eq!(data["BLOG_DESCRIPTION"], "Deep dives");
    assert_eq!(data["BLOG_IMAGE"], "https://acme.example/img/blog.png");
    assert_eq!(data["BLOG_KEYWORDS"], "rust,systems");
    assert_eq!(data["BLOG_URL"], "https://acme.example/blog");
}

#[tokio::test]
async fn list_branding_provider_falls_back_to_web_config_branding() {
    let wc = web_config();
    let mut cfg = config_with_org();
    let mut sources = HashMap::new();
    sources.insert("blog".to_owned(), source_with_branding(None));
    cfg.content_sources = sources;

    let ctx = PageContext::new("blog-list", &wc, &cfg, &());
    let data = DefaultListBrandingProvider
        .provide_page_data(&ctx)
        .await
        .expect("branding data");

    assert_eq!(data["BLOG_NAME"], wc.branding.name);
    assert_eq!(data["BLOG_DESCRIPTION"], wc.branding.description);
    assert_eq!(data["BLOG_IMAGE"], "");
    assert_eq!(data["BLOG_KEYWORDS"], "");
}

#[tokio::test]
async fn homepage_prerenderer_emits_index_spec() {
    let wc = web_config();
    let cfg = config_with_org();
    let dist = std::path::Path::new("/tmp");
    let ctx = PagePrepareContext::new(&wc, &cfg, &(), dist);

    let spec = DefaultHomepagePrerenderer::new()
        .prepare(&ctx)
        .await
        .expect("prepare succeeds")
        .expect("a render spec");

    assert_eq!(spec.output_path, std::path::PathBuf::from("index.html"));
    assert_eq!(spec.base_data["ORG_NAME"], "Acme Docs");
    assert_eq!(spec.base_data["ORG_URL"], "https://acme.example");
}

#[tokio::test]
async fn homepage_prerenderer_errors_when_content_config_absent() {
    let wc = web_config();
    let not_a_config = 7_i64;
    let dist = std::path::Path::new("/tmp");
    let ctx = PagePrepareContext::new(&wc, &not_a_config, &(), dist);

    let err = DefaultHomepagePrerenderer::new()
        .prepare(&ctx)
        .await
        .expect_err("missing ContentConfig must error");

    assert!(matches!(err, ProviderError::Configuration(_)));
}
