//! Database-backed tests for the RSS pipeline: `DefaultRssFeedProvider`
//! branding fallbacks and item mapping against seeded content rows, the
//! end-to-end `generate_feed` file output, and the
//! `generate_feed_with_providers` error arms driven by a stub provider.

use std::fs;
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::Utc;
use std::sync::Arc;
use systemprompt_content::ContentRepository;
use systemprompt_content::models::CreateContentParams;
use systemprompt_database::DbPool;
use systemprompt_generator::{
    DefaultRssFeedProvider, PublishError, generate_feed, generate_feed_with_providers,
};
use systemprompt_identifiers::SourceId;
use systemprompt_provider_contracts::{
    ProviderError, ProviderResult, RssFeedContext, RssFeedItem, RssFeedMetadata, RssFeedProvider,
    RssFeedSpec,
};
use systemprompt_test_fixtures::{
    TestBootstrap, ensure_test_bootstrap, fixture_database_url, fixture_db_pool,
};

static SERIALIZE: Mutex<()> = Mutex::new(());

const WEB_CONFIG_YAML_PATH: &str = "web/config.yaml";

fn install_web_config(boot: &TestBootstrap) {
    fs::write(
        boot.services_path.join(WEB_CONFIG_YAML_PATH),
        crate::config_error_db::web_config_yaml_with_templates_path(""),
    )
    .expect("write web config");
}

fn content_config_yaml(key: &str, source_id: &str, branding: &str) -> String {
    format!(
        r#"content_sources:
  {key}:
    path: "/blog"
    source_id: "{source_id}"
    category_id: "default"
    enabled: true
    sitemap:
      enabled: true
      url_pattern: "/blog/{{slug}}"
      priority: 0.8
      changefreq: "weekly"
      fetch_from: ""
{branding}
  nofeed:
    path: "/nofeed"
    source_id: "{source_id}-off"
    category_id: "default"
    enabled: true
"#
    )
}

fn install_content_config(boot: &TestBootstrap, key: &str, source_id: &str, branding: &str) {
    fs::write(
        boot.services_path.join("content/config.yaml"),
        content_config_yaml(key, source_id, branding),
    )
    .expect("write content config");
}

async fn maybe_db() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

async fn seed_post(db: &DbPool, source_id: &SourceId, slug: &str) {
    let repo = ContentRepository::new(db).expect("content repository");
    let params = CreateContentParams::new(
        slug.to_owned(),
        format!("Feed title {slug}"),
        "feed description".to_owned(),
        "# body".to_owned(),
        source_id.clone(),
    )
    .with_public(true);
    repo.create(&params).await.expect("create content row");
}

async fn cleanup(db: &DbPool, source_id: &SourceId) {
    let repo = ContentRepository::new(db).expect("content repository");
    let _ = repo.delete_by_source(source_id).await;
}

#[tokio::test]
async fn generate_feed_writes_xml_with_seeded_item() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };

    let source_id = SourceId::new("rssgendb");
    cleanup(&db, &source_id).await;
    seed_post(&db, &source_id, "rss-gen-post").await;

    install_web_config(boot);
    install_content_config(
        boot,
        "rssgendb",
        "rssgendb",
        "    branding:\n      name: \"Feed Name\"\n      description: \"Feed Desc\"",
    );
    fs::create_dir_all(boot.app_paths.web().dist()).expect("mkdir dist");

    generate_feed(db.clone(), &boot.app_paths)
        .await
        .expect("generate_feed");

    cleanup(&db, &source_id).await;

    let xml = fs::read_to_string(boot.app_paths.web().dist().join("rssgendb.xml"))
        .expect("read rssgendb.xml feed");
    assert!(
        xml.contains("<title>Feed Name</title>"),
        "channel title must come from source branding: {xml}"
    );
    assert!(
        xml.contains("<description>Feed Desc</description>"),
        "channel description must come from source branding: {xml}"
    );
    assert!(
        xml.contains("/blog/rss-gen-post"),
        "item link must apply the sitemap url_pattern: {xml}"
    );
    assert!(
        xml.contains("<title>Feed title rss-gen-post</title>"),
        "item title must come from the content row: {xml}"
    );
}

#[tokio::test]
async fn rss_provider_branding_falls_back_to_web_branding() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };

    install_web_config(boot);
    install_content_config(boot, "blog", "rssbrandingnone", "");

    let p = DefaultRssFeedProvider::new(db, &boot.app_paths)
        .await
        .expect("provider");
    let ctx = RssFeedContext {
        base_url: "https://example.com",
        source_name: "blog",
    };
    let meta = p.feed_metadata(&ctx).await.expect("metadata");
    assert_eq!(meta.title, "Test Site");
    assert_eq!(meta.description, "a test");
    assert_eq!(meta.language.as_deref(), Some("en"));
}

#[tokio::test]
async fn rss_provider_partial_branding_mixes_source_and_web_defaults() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };

    install_web_config(boot);
    install_content_config(
        boot,
        "blog",
        "rssbrandingpart",
        "    branding:\n      name: \"Only Name\"",
    );

    let p = DefaultRssFeedProvider::new(db, &boot.app_paths)
        .await
        .expect("provider");
    let ctx = RssFeedContext {
        base_url: "https://example.com",
        source_name: "blog",
    };
    let meta = p.feed_metadata(&ctx).await.expect("metadata");
    assert_eq!(meta.title, "Only Name");
    assert_eq!(
        meta.description, "a test",
        "missing branding description must fall back to web branding"
    );
}

#[tokio::test]
async fn rss_provider_feed_specs_skip_sources_without_enabled_sitemap() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };

    install_web_config(boot);
    install_content_config(boot, "blog", "rssspecs", "");

    let p = DefaultRssFeedProvider::new(db, &boot.app_paths)
        .await
        .expect("provider");
    let specs = p.feed_specs();
    assert_eq!(
        specs.len(),
        1,
        "only the sitemap-enabled source emits a feed"
    );
    assert_eq!(specs[0].output_filename, "blog.xml");
    assert_eq!(specs[0].source_id.as_str(), "rssspecs");
}

#[tokio::test]
async fn rss_provider_fetch_items_maps_slug_through_url_pattern() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };

    let source_id = SourceId::new("rssfetchdb");
    cleanup(&db, &source_id).await;
    seed_post(&db, &source_id, "rss-fetch-post").await;

    install_web_config(boot);
    install_content_config(boot, "blog", "rssfetchdb", "");

    let p = DefaultRssFeedProvider::new(db.clone(), &boot.app_paths)
        .await
        .expect("provider");
    let ctx = RssFeedContext {
        base_url: "https://example.com",
        source_name: "rssfetchdb",
    };
    let items = p.fetch_items(&ctx, 10).await.expect("items");

    cleanup(&db, &source_id).await;

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].link, "https://example.com/blog/rss-fetch-post");
    assert_eq!(items[0].guid, items[0].link);
    assert_eq!(items[0].title, "Feed title rss-fetch-post");
}

struct StubRssProvider {
    metadata_fails: bool,
    items_fail: bool,
}

#[async_trait]
impl RssFeedProvider for StubRssProvider {
    fn provider_id(&self) -> &'static str {
        "stub-rss"
    }

    fn feed_specs(&self) -> Vec<RssFeedSpec> {
        vec![RssFeedSpec {
            source_id: SourceId::new("stub"),
            max_items: 3,
            output_filename: "stub.xml".to_owned(),
        }]
    }

    async fn feed_metadata(&self, _ctx: &RssFeedContext<'_>) -> ProviderResult<RssFeedMetadata> {
        if self.metadata_fails {
            return Err(ProviderError::Configuration("meta boom".into()));
        }
        Ok(RssFeedMetadata {
            title: "Stub".to_owned(),
            link: "https://stub.example".to_owned(),
            description: "stub feed".to_owned(),
            language: None,
        })
    }

    async fn fetch_items(
        &self,
        _ctx: &RssFeedContext<'_>,
        _limit: i64,
    ) -> ProviderResult<Vec<RssFeedItem>> {
        if self.items_fail {
            return Err(ProviderError::RenderFailed("items boom".into()));
        }
        Ok(vec![RssFeedItem {
            title: "Stub Item".to_owned(),
            link: "https://stub.example/item".to_owned(),
            description: "stub item".to_owned(),
            pub_date: Utc::now(),
            guid: "https://stub.example/item".to_owned(),
            author: None,
        }])
    }
}

#[tokio::test]
async fn generate_feed_with_providers_builds_feed_from_stub() {
    let _boot = ensure_test_bootstrap();
    let providers: Vec<Arc<dyn RssFeedProvider>> = vec![Arc::new(StubRssProvider {
        metadata_fails: false,
        items_fail: false,
    })];
    let feeds = generate_feed_with_providers(&providers)
        .await
        .expect("feeds");
    assert_eq!(feeds.len(), 1);
    assert_eq!(feeds[0].filename, "stub.xml");
    assert_eq!(feeds[0].item_count, 1);
    assert!(feeds[0].xml.contains("<title>Stub</title>"));
    assert!(feeds[0].xml.contains("https://stub.example/item"));
}

#[tokio::test]
async fn generate_feed_with_providers_propagates_metadata_failure() {
    let _boot = ensure_test_bootstrap();
    let providers: Vec<Arc<dyn RssFeedProvider>> = vec![Arc::new(StubRssProvider {
        metadata_fails: true,
        items_fail: false,
    })];
    let err = generate_feed_with_providers(&providers)
        .await
        .expect_err("metadata failure");
    assert!(
        matches!(err, PublishError::ProviderFailed { ref provider_id, .. } if provider_id == "stub-rss"),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn generate_feed_with_providers_propagates_fetch_failure() {
    let _boot = ensure_test_bootstrap();
    let providers: Vec<Arc<dyn RssFeedProvider>> = vec![Arc::new(StubRssProvider {
        metadata_fails: false,
        items_fail: true,
    })];
    let err = generate_feed_with_providers(&providers)
        .await
        .expect_err("fetch failure");
    assert!(
        matches!(err, PublishError::ProviderFailed { ref cause, .. } if cause.contains("items boom")),
        "unexpected error: {err:?}"
    );
}

fn tempdir_paths(tmp: &tempfile::TempDir) -> systemprompt_models::AppPaths {
    let p = tmp.path().to_string_lossy().to_string();
    systemprompt_models::AppPaths::from_profile(&systemprompt_models::profile::PathsConfig {
        system: p.clone(),
        services: p.clone(),
        bin: p.clone(),
        web_path: Some(p.clone()),
        storage: Some(p),
        geoip_database: None,
    })
    .expect("paths")
}

#[tokio::test]
async fn rss_provider_missing_content_config_is_read_error() {
    let _boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };
    let tmp = tempfile::TempDir::new().unwrap();
    let err = DefaultRssFeedProvider::new(db, &tempdir_paths(&tmp))
        .await
        .expect_err("missing content config");
    assert!(
        matches!(err, PublishError::ContentConfigRead { .. }),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn rss_provider_malformed_content_config_is_parse_error() {
    let _boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };
    let tmp = tempfile::TempDir::new().unwrap();
    let paths = tempdir_paths(&tmp);
    let cfg = paths.system().content_config().to_path_buf();
    fs::create_dir_all(cfg.parent().unwrap()).unwrap();
    fs::write(&cfg, "content_sources: [broken").unwrap();
    let err = DefaultRssFeedProvider::new(db, &paths)
        .await
        .expect_err("malformed content config");
    assert!(
        matches!(err, PublishError::ContentConfigParse { .. }),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn rss_provider_fetch_items_with_closed_pool_is_render_failed() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    if fixture_database_url().is_err() {
        return;
    }

    install_web_config(boot);
    install_content_config(boot, "blog", "rssclosedsrc", "");

    let p = DefaultRssFeedProvider::new(
        systemprompt_test_fixtures::closed_db_pool().await,
        &boot.app_paths,
    )
    .await
    .expect("provider construction only reads config files");
    let ctx = RssFeedContext {
        base_url: "https://example.com",
        source_name: "rssclosedsrc",
    };
    let err = p.fetch_items(&ctx, 5).await.expect_err("closed pool");
    assert!(
        matches!(err, ProviderError::RenderFailed(ref m) if m.contains("Failed to fetch content")),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn rss_provider_fetch_items_defaults_url_pattern_without_sitemap() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };

    let source_id = SourceId::new("rssnositemap-off");
    cleanup(&db, &source_id).await;
    seed_post(&db, &source_id, "plain-slug-post").await;

    install_web_config(boot);
    install_content_config(boot, "blog", "rssnositemap", "");

    let p = DefaultRssFeedProvider::new(db.clone(), &boot.app_paths)
        .await
        .expect("provider");
    let ctx = RssFeedContext {
        base_url: "https://example.com",
        source_name: "rssnositemap-off",
    };
    let items = p.fetch_items(&ctx, 5).await.expect("items");

    cleanup(&db, &source_id).await;

    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].link, "https://example.com/plain-slug-post",
        "source without sitemap must use the default slug pattern"
    );
}
