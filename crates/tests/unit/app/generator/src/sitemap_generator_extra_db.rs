//! Additional `generate_sitemap` arms: disabled sources and disabled sitemap
//! sections are skipped, rows in a locale outside `supported_locales` are
//! excluded from hreflang alternates, and a closed pool surfaces the fetch
//! error path.

use std::fs;
use std::sync::Mutex;

use systemprompt_content::ContentRepository;
use systemprompt_content::models::CreateContentParams;
use systemprompt_database::DbPool;
use systemprompt_generator::{PublishError, generate_sitemap};
use systemprompt_identifiers::{LocaleCode, SourceId};
use systemprompt_test_fixtures::{
    TestBootstrap, closed_db_pool, ensure_test_bootstrap, fixture_database_url, fixture_db_pool,
};

static SERIALIZE: Mutex<()> = Mutex::new(());

fn content_config_yaml(tag: &str) -> String {
    format!(
        r#"content_sources:
  blog:
    path: "/blog"
    source_id: "{tag}"
    category_id: "default"
    enabled: true
    sitemap:
      enabled: true
      url_pattern: "/blog/{{slug}}"
      priority: 0.8
      changefreq: "weekly"
      fetch_from: ""
  off_source:
    path: "/off"
    source_id: "{tag}-off"
    category_id: "default"
    enabled: false
    sitemap:
      enabled: true
      url_pattern: "/off/{{slug}}"
      priority: 0.5
      changefreq: "weekly"
      fetch_from: ""
  nositemap:
    path: "/nos"
    source_id: "{tag}-nos"
    category_id: "default"
    enabled: true
    sitemap:
      enabled: false
      url_pattern: "/nos/{{slug}}"
      priority: 0.5
      changefreq: "weekly"
      fetch_from: ""
"#
    )
}

fn install_config(boot: &TestBootstrap, tag: &str) {
    fs::write(
        boot.services_path.join("web/config.yaml"),
        crate::config_error_db::web_config_yaml_with_templates_path(""),
    )
    .expect("write web config");
    fs::write(
        boot.services_path.join("content/config.yaml"),
        content_config_yaml(tag),
    )
    .expect("write content config");
    fs::create_dir_all(boot.app_paths.web().dist()).expect("mkdir dist");
}

async fn maybe_db() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

async fn seed(db: &DbPool, source_id: &SourceId, slug: &str, locale: &str) {
    let repo = ContentRepository::new(db).expect("content repository");
    let params = CreateContentParams::new(
        slug.to_owned(),
        "Title".to_owned(),
        "desc".to_owned(),
        "# body".to_owned(),
        source_id.clone(),
    )
    .with_locale(LocaleCode::new(locale))
    .with_public(true);
    repo.create(&params).await.expect("create content row");
}

async fn cleanup(db: &DbPool, tags: &[&str]) {
    let repo = ContentRepository::new(db).expect("content repository");
    for tag in tags {
        let _ = repo.delete_by_source(&SourceId::new(*tag)).await;
    }
}

#[tokio::test]
async fn generate_sitemap_skips_disabled_sources_and_disabled_sitemaps() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };

    let tag = "smapextraskip";
    let off = format!("{tag}-off");
    let nos = format!("{tag}-nos");
    cleanup(&db, &[tag, &off, &nos]).await;
    seed(&db, &SourceId::new(tag), "visible-post", "en").await;
    seed(&db, &SourceId::new(off.as_str()), "disabled-post", "en").await;
    seed(&db, &SourceId::new(nos.as_str()), "nositemap-post", "en").await;

    install_config(boot, tag);
    generate_sitemap(db.clone(), &boot.app_paths)
        .await
        .expect("generate_sitemap");

    let xml = fs::read_to_string(boot.app_paths.web().dist().join("sitemap.xml"))
        .expect("read sitemap.xml");
    cleanup(&db, &[tag, &off, &nos]).await;

    assert!(
        xml.contains("/blog/visible-post"),
        "enabled source slug missing:\n{xml}"
    );
    assert!(
        !xml.contains("disabled-post"),
        "disabled source leaked into sitemap:\n{xml}"
    );
    assert!(
        !xml.contains("nositemap-post"),
        "sitemap-disabled source leaked into sitemap:\n{xml}"
    );
}

#[tokio::test]
async fn generate_sitemap_excludes_unsupported_locales_from_alternates() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };

    let tag = "smapextradeloc";
    cleanup(&db, &[tag]).await;
    seed(&db, &SourceId::new(tag), "multi-locale-post", "en").await;
    seed(&db, &SourceId::new(tag), "multi-locale-post", "de").await;

    install_config(boot, tag);
    generate_sitemap(db.clone(), &boot.app_paths)
        .await
        .expect("generate_sitemap");

    let xml = fs::read_to_string(boot.app_paths.web().dist().join("sitemap.xml"))
        .expect("read sitemap.xml");
    cleanup(&db, &[tag]).await;

    assert!(
        xml.contains("/blog/multi-locale-post"),
        "slug missing:\n{xml}"
    );
    assert!(
        !xml.contains("hreflang=\"de\""),
        "unsupported locale must not appear as hreflang alternate:\n{xml}"
    );
    assert!(
        xml.contains("hreflang=\"x-default\""),
        "x-default alternate missing:\n{xml}"
    );
}

#[tokio::test]
async fn generate_sitemap_with_closed_pool_is_fetch_error() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    if fixture_database_url().is_err() {
        return;
    }

    install_config(boot, "smapextraclosed");
    let err = generate_sitemap(closed_db_pool().await, &boot.app_paths)
        .await
        .expect_err("closed pool must fail sitemap fetch");
    assert!(
        matches!(err, PublishError::FetchFailed { .. }),
        "unexpected error: {err:?}"
    );
}
