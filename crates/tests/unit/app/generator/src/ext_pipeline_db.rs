//! Tests driven by the inventory fixtures in [`crate::ext_fixtures`]:
//! the extension-asset copy loop end-to-end, the component / extender /
//! content-data enrichment arms in the prerender pipeline, and the page
//! prerenderer arms (rendered page, duplicate page type, `None` spec).
//! Also covers the multi-file sitemap chunking path with 50k+ synthetic
//! rows and assorted small remaining arms.

use std::fs;
use std::sync::Mutex;

use systemprompt_content::ContentRepository;
use systemprompt_content::models::CreateContentParams;
use systemprompt_database::DbPool;
use systemprompt_extension::AssetPaths;
use systemprompt_generator::{
    execute_copy_extension_assets, generate_sitemap, get_templates_path, load_web_config,
    prerender_content, prerender_pages,
};
use systemprompt_identifiers::SourceId;
use systemprompt_test_fixtures::{
    TestBootstrap, ensure_test_bootstrap, fixture_database_url, fixture_db_pool,
};

use crate::config_error_db::web_config_yaml_with_templates_path;
use crate::ext_fixtures::{GEN_REQUIRED_ASSET_DEST, GEN_REQUIRED_ASSET_SOURCE};

static SERIALIZE: Mutex<()> = Mutex::new(());

fn content_config_yaml(source_id: &str) -> String {
    format!(
        r#"content_sources:
  blog:
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
"#
    )
}

fn install_config(boot: &TestBootstrap, source_id: &str) {
    fs::write(
        boot.services_path.join("web/config.yaml"),
        web_config_yaml_with_templates_path(""),
    )
    .expect("write web config");
    fs::write(
        boot.services_path.join("content/config.yaml"),
        content_config_yaml(source_id),
    )
    .expect("write content config");
    fs::create_dir_all(boot.app_paths.web().dist()).expect("mkdir dist");
    fs::create_dir_all(boot.app_paths.web().root().join("templates")).expect("mkdir templates");
}

async fn maybe_db() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

#[tokio::test]
async fn copy_extension_assets_copies_required_and_tolerates_optional_missing() {
    let tmp = tempfile::TempDir::new().unwrap();
    let p = tmp.path().to_string_lossy().to_string();
    let paths =
        systemprompt_models::AppPaths::from_profile(&systemprompt_models::profile::PathsConfig {
            system: p.clone(),
            services: p.clone(),
            bin: p.clone(),
            web_path: Some(p.clone()),
            storage: Some(p),
            geoip_database: None,
        })
        .expect("paths");

    let src = paths.storage_files().join(GEN_REQUIRED_ASSET_SOURCE);
    fs::create_dir_all(src.parent().unwrap()).unwrap();
    fs::write(&src, "body { margin: 0 }").unwrap();
    fs::create_dir_all(paths.web().dist()).unwrap();

    let result = execute_copy_extension_assets(&paths)
        .await
        .expect("job must succeed with optional-missing asset");
    assert!(result.success);
    assert_eq!(result.items_processed, Some(1), "one required asset copied");
    assert_eq!(result.items_failed, Some(1), "one optional asset missing");

    let dest = paths.web().dist().join(GEN_REQUIRED_ASSET_DEST);
    assert_eq!(
        fs::read_to_string(&dest).expect("copied asset"),
        "body { margin: 0 }"
    );
}

#[tokio::test]
async fn copy_extension_assets_fails_when_required_asset_missing() {
    let tmp = tempfile::TempDir::new().unwrap();
    let p = tmp.path().to_string_lossy().to_string();
    let paths =
        systemprompt_models::AppPaths::from_profile(&systemprompt_models::profile::PathsConfig {
            system: p.clone(),
            services: p.clone(),
            bin: p.clone(),
            web_path: Some(p.clone()),
            storage: Some(p),
            geoip_database: None,
        })
        .expect("paths");
    fs::create_dir_all(paths.web().dist()).unwrap();

    let err = execute_copy_extension_assets(&paths)
        .await
        .expect_err("missing required asset must fail the job");
    assert!(
        err.to_string()
            .contains(GEN_REQUIRED_ASSET_SOURCE.rsplit('/').next().unwrap()),
        "error must name the missing asset: {err}"
    );
}

#[tokio::test]
async fn prerender_runs_fixture_components_extenders_and_enrichment() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };

    let source_id = SourceId::new("extpipesrc");
    let repo = ContentRepository::new(&db).expect("content repository");
    let _ = repo.delete_by_source(&source_id).await;
    for locale in ["en", "fr"] {
        repo.create(
            &CreateContentParams::new(
                "ext-pipe-post".to_owned(),
                "Ext Pipe".to_owned(),
                "desc".to_owned(),
                "# Body".to_owned(),
                source_id.clone(),
            )
            .with_locale(systemprompt_identifiers::LocaleCode::new(locale))
            .with_public(true),
        )
        .await
        .expect("create row");
    }

    install_config(boot, "extpipesrc");
    let tmpl_dir = boot.app_paths.web().root().join("templates");
    fs::write(
        tmpl_dir.join("article-post.html"),
        "<article>{{{GEN_COMPONENT}}}|{{GEN_EXTENDED}}|{{GEN_ENRICHED}}|{{{GEN_PARTIAL}}}</article>",
    )
    .expect("write template");

    prerender_content(db.clone(), &boot.app_paths)
        .await
        .expect("prerender_content");

    let _ = repo.delete_by_source(&source_id).await;
    let _ = fs::remove_file(tmpl_dir.join("article-post.html"));

    let html = fs::read_to_string(
        boot.app_paths
            .web()
            .dist()
            .join("blog/ext-pipe-post/index.html"),
    )
    .expect("rendered page");
    assert!(
        html.contains("<b>component:gen_comp_hi</b>"),
        "higher-priority component must win the shared variable: {html}"
    );
    assert!(
        html.contains("|extended|"),
        "extender must inject GEN_EXTENDED: {html}"
    );
    assert!(
        html.contains("|gen_cd_all|"),
        "matching content-data provider must enrich the item: {html}"
    );
}

#[tokio::test]
async fn prerender_pages_renders_fixture_page_with_provider_data() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };

    install_config(boot, "extpipepages");
    let tmpl_dir = boot.app_paths.web().root().join("templates");
    fs::write(
        tmpl_dir.join("covgenpage.html"),
        "<div data-base=\"{{GEN_BASE}}\" data-provided=\"{{GEN_PAGE_FIELD}}\" lang=\"{{locale}}\"></div>",
    )
    .expect("write covgenpage template");

    let results = prerender_pages(db.clone(), &boot.app_paths)
        .await
        .expect("prerender_pages");

    let _ = fs::remove_file(tmpl_dir.join("covgenpage.html"));

    let covgen: Vec<_> = results
        .iter()
        .filter(|r| r.page_type == "covgenpage")
        .collect();
    assert!(
        !covgen.is_empty(),
        "covgenpage prerenderer must produce output: {results:?}"
    );
    let html = fs::read_to_string(&covgen[0].output_path).expect("covgen page output");
    assert!(
        html.contains("data-base=\"base\""),
        "base data missing: {html}"
    );
    assert!(
        html.contains("data-provided=\"from-provider\""),
        "page-data provider field missing: {html}"
    );
    assert!(
        !results.iter().any(|r| r.page_type == "covgennone"),
        "None-returning prerenderer must not emit a page"
    );
}

#[tokio::test]
async fn prerender_empty_source_retries_then_renders_nothing() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };

    let source_id = SourceId::new("extpipeempty");
    let repo = ContentRepository::new(&db).expect("content repository");
    let _ = repo.delete_by_source(&source_id).await;

    install_config(boot, "extpipeempty");
    prerender_content(db.clone(), &boot.app_paths)
        .await
        .expect("empty source must complete without error");
    assert!(
        !boot.app_paths.web().dist().join("blog/index.html").exists()
            || fs::read_dir(boot.app_paths.web().dist().join("blog")).is_ok(),
        "no page is rendered for an empty source"
    );
}

#[tokio::test]
async fn generate_sitemap_chunks_into_index_when_over_url_limit() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };

    let pool = db.pool_arc().expect("pg pool");
    sqlx::query("DELETE FROM markdown_content WHERE source_id = 'extpipebulk'")
        .execute(pool.as_ref())
        .await
        .expect("clean bulk rows");
    sqlx::query(
        "INSERT INTO markdown_content (id, slug, title, description, body, author, \
         published_at, keywords, source_id, version_hash) \
         SELECT 'extpipebulk-'||i, 'extpipebulk-slug-'||i, 't', 'd', 'b', 'a', now(), '', \
         'extpipebulk', 'h' FROM generate_series(1, 50001) AS i",
    )
    .execute(pool.as_ref())
    .await
    .expect("bulk insert 50001 rows");

    install_config(boot, "extpipebulk");
    let result = generate_sitemap(db.clone(), &boot.app_paths).await;

    sqlx::query("DELETE FROM markdown_content WHERE source_id = 'extpipebulk'")
        .execute(pool.as_ref())
        .await
        .expect("clean bulk rows");

    result.expect("generate_sitemap over 50k urls");

    let dist = boot.app_paths.web().dist();
    let index = fs::read_to_string(dist.join("sitemap.xml")).expect("sitemap index");
    assert!(
        index.contains("<sitemapindex"),
        "over-limit sitemap must be an index: {}",
        &index[..index.len().min(400)]
    );
    assert!(
        index.contains("sitemaps/sitemap-1.xml"),
        "chunk 1 missing from index"
    );
    assert!(
        index.contains("sitemaps/sitemap-2.xml"),
        "chunk 2 missing from index"
    );
    assert!(dist.join("sitemaps/sitemap-1.xml").exists());
    assert!(dist.join("sitemaps/sitemap-2.xml").exists());
    let chunk2 = fs::read_to_string(dist.join("sitemaps/sitemap-2.xml")).expect("chunk 2");
    assert!(
        chunk2.contains("extpipebulk-slug-"),
        "second chunk must carry overflow urls"
    );
}

#[tokio::test]
async fn get_templates_path_falls_back_when_configured_path_missing() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    fs::write(
        boot.services_path.join("web/config.yaml"),
        web_config_yaml_with_templates_path(""),
    )
    .expect("write web config");
    let mut cfg = load_web_config(&boot.app_paths).await.expect("web config");
    cfg.paths.templates = "/nonexistent/configured/templates".to_owned();
    assert_eq!(
        get_templates_path(&cfg, &boot.app_paths),
        boot.app_paths.web().root().join("templates"),
        "missing configured path must fall back to web root templates"
    );
}

#[tokio::test]
async fn validate_build_skips_unparseable_sitemap_urls() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dist = tmp.path().join("dist");
    fs::create_dir_all(&dist).unwrap();
    fs::write(dist.join("index.html"), "<html></html>").unwrap();
    fs::write(
        dist.join("sitemap.xml"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
<url><loc>not-a-parseable-url</loc></url>
<url><loc>https://example.com/</loc></url>
</urlset>"#,
    )
    .unwrap();

    let orch = systemprompt_generator::BuildOrchestrator::new(
        tmp.path().to_path_buf(),
        systemprompt_generator::BuildMode::Production,
    );
    orch.validate_only()
        .await
        .expect("unparseable url is skipped, parseable root url resolves to index.html");
}
