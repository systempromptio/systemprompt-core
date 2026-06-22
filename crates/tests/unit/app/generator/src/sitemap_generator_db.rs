//! Database-backed tests for the public `generate_sitemap` entry point that
//! drive `sitemap::generator`'s per-slug URL collection
//! (`fetch_urls_from_database`) and multi-locale parent-route synthesis
//! (`build_parent_urls`) end-to-end against real `markdown_content` rows.
//!
//! These previously had zero coverage: the existing pipeline tests run against
//! an empty content database, so the per-slug loop and the hreflang-alternate
//! branches never executed. Here we insert `public = true` rows in two locales
//! for a configured source, run the generator, and assert on the concrete
//! `sitemap.xml` URL entries it emits (loc, locale prefix, hreflang
//! alternates, and the parent-route URLs).

use std::fs;
use std::sync::Mutex;

use systemprompt_content::ContentRepository;
use systemprompt_content::models::CreateContentParams;
use systemprompt_database::DbPool;
use systemprompt_generator::generate_sitemap;
use systemprompt_identifiers::{LocaleCode, SourceId};
use systemprompt_test_fixtures::{
    TestBootstrap, ensure_test_bootstrap, fixture_database_url, fixture_db_pool,
};

/// Serialises this module's tests so each one can write the shared
/// `web/config.yaml` + `content/config.yaml`, run the generator, and read back
/// `sitemap.xml` without a sibling test in this module overwriting the config
/// mid-flight.
static SERIALIZE: Mutex<()> = Mutex::new(());

const WEB_CONFIG_TWO_LOCALES_YAML: &str = r##"
paths:
  templates: ""
  assets: ""
branding:
  name: testsite
  title: Test Site
  description: a test
  copyright: "(c) test"
  themeColor: "#000000"
  display_sitename: true
  twitter_handle: "@test"
  logo:
    primary: {}
  favicon: "/favicon.ico"
i18n:
  default_locale: en
  supported_locales: [en, fr]
fonts:
  body:
    family: "Inter"
    fallback: "sans-serif"
  heading:
    family: "Inter"
    fallback: "sans-serif"
colors:
  light:
    primary: { hsl: "0 0% 0%", rgb: [0, 0, 0] }
    secondary: { hsl: "0 0% 0%", rgb: [0, 0, 0] }
    success: "#0f0"
    warning: "#ff0"
    error: "#f00"
    surface:
      default: "#fff"
      dark: "#000"
      variant: "#eee"
      secondaryContainer: "#ddd"
      errorContainer: "#fdd"
    text:
      primary: "#000"
      secondary: "#333"
      inverted: "#fff"
      disabled: "#999"
    background:
      default: "#fff"
      dark: "#000"
    border:
      default: "#ccc"
      dark: "#222"
      outline: "#888"
  dark:
    primary: { hsl: "0 0% 100%", rgb: [255, 255, 255] }
    secondary: { hsl: "0 0% 100%", rgb: [255, 255, 255] }
    success: "#0f0"
    warning: "#ff0"
    error: "#f00"
    surface:
      default: "#000"
      dark: "#000"
      variant: "#111"
      secondaryContainer: "#222"
      errorContainer: "#311"
    text:
      primary: "#fff"
      secondary: "#ccc"
      inverted: "#000"
      disabled: "#666"
    background:
      default: "#000"
      dark: "#000"
    border:
      default: "#333"
      dark: "#222"
      outline: "#666"
typography:
  sizes: { xs: "0.75rem", sm: "0.875rem", md: "1rem", lg: "1.125rem", xl: "1.5rem", xxl: "2rem" }
  weights: { regular: 400, medium: 500, semibold: 600, bold: 700 }
spacing: { xs: "4px", sm: "8px", md: "16px", lg: "24px", xl: "32px", xxl: "48px" }
radius: { xs: "2px", sm: "4px", md: "8px", lg: "12px", xl: "16px", xxl: "24px", round: "9999px" }
shadows:
  light: { sm: "x", md: "x", lg: "x", accent: "x" }
  dark: { sm: "x", md: "x", lg: "x", accent: "x" }
animation: { fast: "100ms", normal: "200ms", slow: "300ms" }
zIndex: { base: 0, content: 1, navigation: 2, modal: 3, tooltip: 4 }
layout:
  headerHeight: "60px"
  sidebarLeft: { width: "240px", minWidth: "200px", maxWidth: "300px" }
  sidebarRight: { width: "240px", minWidth: "200px", maxWidth: "300px" }
  navHeight: "48px"
  contentMaxWidth: "1200px"
card:
  radius: { default: "8px", cut: "0px" }
  padding: { sm: "8px", md: "16px", lg: "24px" }
  gradient: { start: "#fff", mid: "#eee", end: "#ddd" }
mobile:
  spacing: { xs: "4px", sm: "8px", md: "16px", lg: "24px", xl: "32px", xxl: "48px" }
  typography:
    sizes: { xs: "0.75rem", sm: "0.875rem", md: "1rem", lg: "1.125rem", xl: "1.5rem", xxl: "2rem" }
  layout: { headerHeight: "56px", navHeight: "48px" }
  card:
    padding: { sm: "8px", md: "12px", lg: "16px" }
touchTargets: { default: "44px", sm: "32px", lg: "56px" }
"##;

const CONTENT_CONFIG_WITH_SOURCE_YAML: &str = r##"
content_sources:
  blog:
    path: "/blog"
    source_id: "sitemapdbtest"
    category_id: "default"
    enabled: true
    sitemap:
      enabled: true
      url_pattern: "/blog/{slug}"
      priority: 0.8
      changefreq: "weekly"
      fetch_from: ""
      parent_route:
        enabled: true
        url: "/blog"
        priority: 0.9
        changefreq: "daily"
    branding:
      name: "Blog Feed"
      description: "Blog posts"
"##;

const TEST_SOURCE_ID: &str = "sitemapdbtest";
const TEST_SLUG: &str = "sitemap-db-fixture-post";

fn install_config(boot: &TestBootstrap) {
    fs::create_dir_all(boot.services_path.join("web")).ok();
    fs::create_dir_all(boot.services_path.join("content")).ok();
    fs::write(
        boot.services_path.join("web/config.yaml"),
        WEB_CONFIG_TWO_LOCALES_YAML,
    )
    .expect("write web config");
    fs::write(
        boot.services_path.join("content/config.yaml"),
        CONTENT_CONFIG_WITH_SOURCE_YAML,
    )
    .expect("write content config");
    fs::create_dir_all(boot.app_paths.web().dist()).ok();
}

async fn maybe_db() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

async fn seed_two_locale_post(db: &DbPool) {
    let repo = ContentRepository::new(db).expect("content repository");
    let source_id = SourceId::new(TEST_SOURCE_ID);

    repo.delete_by_source(&source_id)
        .await
        .expect("clean source rows");

    for locale in ["en", "fr"] {
        let params = CreateContentParams::new(
            TEST_SLUG.to_owned(),
            "Fixture Post".to_owned(),
            "Fixture description".to_owned(),
            "# Fixture body".to_owned(),
            source_id.clone(),
        )
        .with_locale(LocaleCode::new(locale))
        .with_public(true);
        repo.create(&params).await.expect("create content row");
    }
}

async fn cleanup(db: &DbPool) {
    let repo = ContentRepository::new(db).expect("content repository");
    let source_id = SourceId::new(TEST_SOURCE_ID);
    let _ = repo.delete_by_source(&source_id).await;
}

#[tokio::test]
async fn generate_sitemap_emits_per_slug_urls_with_hreflang_alternates() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };

    seed_two_locale_post(&db).await;
    install_config(boot);

    generate_sitemap(db.clone(), &boot.app_paths)
        .await
        .expect("generate_sitemap");

    let xml = fs::read_to_string(boot.app_paths.web().dist().join("sitemap.xml"))
        .expect("read sitemap.xml");

    cleanup(&db).await;

    // English (default locale) gets no prefix; the slug URL must appear.
    assert!(
        xml.contains(&format!("/blog/{TEST_SLUG}</loc>")),
        "english slug loc missing in:\n{xml}"
    );
    // French is a non-default supported locale, so it carries a `/fr` prefix.
    assert!(
        xml.contains(&format!("/fr/blog/{TEST_SLUG}</loc>")),
        "french prefixed slug loc missing in:\n{xml}"
    );
    // Per-URL hreflang alternates for both locales plus x-default.
    assert!(
        xml.contains("hreflang=\"en\""),
        "missing en hreflang alternate in:\n{xml}"
    );
    assert!(
        xml.contains("hreflang=\"fr\""),
        "missing fr hreflang alternate in:\n{xml}"
    );
    assert!(
        xml.contains("hreflang=\"x-default\""),
        "missing x-default hreflang alternate in:\n{xml}"
    );
    // Slug URLs carry the source's configured priority (0.8) and changefreq.
    assert!(
        xml.contains("<priority>0.8</priority>"),
        "missing slug priority in:\n{xml}"
    );
    assert!(
        xml.contains("<changefreq>weekly</changefreq>"),
        "missing slug changefreq in:\n{xml}"
    );
}

#[tokio::test]
async fn generate_sitemap_emits_parent_route_urls_for_each_locale() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };

    seed_two_locale_post(&db).await;
    install_config(boot);

    generate_sitemap(db.clone(), &boot.app_paths)
        .await
        .expect("generate_sitemap");

    let xml = fs::read_to_string(boot.app_paths.web().dist().join("sitemap.xml"))
        .expect("read sitemap.xml");

    cleanup(&db).await;

    // parent_route.url = "/blog" is emitted once per supported locale: the
    // default (no prefix) and the French (`/fr`) variant.
    assert!(
        xml.contains("/blog</loc>"),
        "missing default parent-route loc in:\n{xml}"
    );
    assert!(
        xml.contains("/fr/blog</loc>"),
        "missing french parent-route loc in:\n{xml}"
    );
    // The parent route carries its own priority (0.9) and changefreq (daily).
    assert!(
        xml.contains("<priority>0.9</priority>"),
        "missing parent-route priority in:\n{xml}"
    );
    assert!(
        xml.contains("<changefreq>daily</changefreq>"),
        "missing parent-route changefreq in:\n{xml}"
    );
}

#[tokio::test]
async fn generate_sitemap_excludes_non_public_rows() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };

    let repo = ContentRepository::new(&db).expect("content repository");
    let source_id = SourceId::new(TEST_SOURCE_ID);
    repo.delete_by_source(&source_id)
        .await
        .expect("clean source rows");

    let private_slug = "sitemap-db-private-post";
    let params = CreateContentParams::new(
        private_slug.to_owned(),
        "Private Post".to_owned(),
        "hidden".to_owned(),
        "# hidden".to_owned(),
        source_id.clone(),
    )
    .with_public(false);
    repo.create(&params).await.expect("create private row");

    install_config(boot);

    generate_sitemap(db.clone(), &boot.app_paths)
        .await
        .expect("generate_sitemap");

    let xml = fs::read_to_string(boot.app_paths.web().dist().join("sitemap.xml"))
        .expect("read sitemap.xml");

    cleanup(&db).await;

    assert!(
        !xml.contains(private_slug),
        "non-public slug leaked into sitemap:\n{xml}"
    );
    // The parent route is config-driven, not row-driven, so it is still present.
    assert!(
        xml.contains("/blog</loc>"),
        "parent-route loc missing in:\n{xml}"
    );
}
