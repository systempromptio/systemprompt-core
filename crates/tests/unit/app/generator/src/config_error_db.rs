//! Error-arm tests for the config loaders shared by the prerender, sitemap,
//! and RSS pipelines: missing / malformed `content/config.yaml`, missing /
//! malformed `web/config.yaml`, a configured-but-absent templates path, and
//! the `get_templates_path` fallback branches.

use std::fs;
use std::sync::Mutex;

use systemprompt_database::DbPool;
use systemprompt_generator::{
    DefaultSitemapProvider, PublishError, generate_sitemap, get_templates_path, load_web_config,
    prerender_content,
};
use systemprompt_models::AppPaths;
use systemprompt_models::profile::PathsConfig;
use systemprompt_test_fixtures::{
    TestBootstrap, closed_db_pool, ensure_test_bootstrap, fixture_database_url, fixture_db_pool,
};

static SERIALIZE: Mutex<()> = Mutex::new(());

pub(crate) fn web_config_yaml_with_templates_path(templates: &str) -> String {
    format!(
        r##"
paths:
  templates: "{templates}"
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
    primary: {{}}
  favicon: "/favicon.ico"
fonts:
  body: {{ family: "Inter", fallback: "sans-serif" }}
  heading: {{ family: "Inter", fallback: "sans-serif" }}
colors:
  light:
    primary: {{ hsl: "0 0% 0%", rgb: [0, 0, 0] }}
    secondary: {{ hsl: "0 0% 0%", rgb: [0, 0, 0] }}
    success: "#0f0"
    warning: "#ff0"
    error: "#f00"
    surface: {{ default: "#fff", dark: "#000", variant: "#eee", secondaryContainer: "#ddd", errorContainer: "#fdd" }}
    text: {{ primary: "#000", secondary: "#333", inverted: "#fff", disabled: "#999" }}
    background: {{ default: "#fff", dark: "#000" }}
    border: {{ default: "#ccc", dark: "#222", outline: "#888" }}
  dark:
    primary: {{ hsl: "0 0% 100%", rgb: [255, 255, 255] }}
    secondary: {{ hsl: "0 0% 100%", rgb: [255, 255, 255] }}
    success: "#0f0"
    warning: "#ff0"
    error: "#f00"
    surface: {{ default: "#000", dark: "#000", variant: "#111", secondaryContainer: "#222", errorContainer: "#311" }}
    text: {{ primary: "#fff", secondary: "#ccc", inverted: "#000", disabled: "#666" }}
    background: {{ default: "#000", dark: "#000" }}
    border: {{ default: "#333", dark: "#222", outline: "#666" }}
typography:
  sizes: {{ xs: "0.75rem", sm: "0.875rem", md: "1rem", lg: "1.125rem", xl: "1.5rem", xxl: "2rem" }}
  weights: {{ regular: 400, medium: 500, semibold: 600, bold: 700 }}
spacing: {{ xs: "4px", sm: "8px", md: "16px", lg: "24px", xl: "32px", xxl: "48px" }}
radius: {{ xs: "2px", sm: "4px", md: "8px", lg: "12px", xl: "16px", xxl: "24px", round: "9999px" }}
shadows:
  light: {{ sm: "x", md: "x", lg: "x", accent: "x" }}
  dark: {{ sm: "x", md: "x", lg: "x", accent: "x" }}
animation: {{ fast: "100ms", normal: "200ms", slow: "300ms" }}
zIndex: {{ base: 0, content: 1, navigation: 2, modal: 3, tooltip: 4 }}
layout:
  headerHeight: "60px"
  sidebarLeft: {{ width: "240px", minWidth: "200px", maxWidth: "300px" }}
  sidebarRight: {{ width: "240px", minWidth: "200px", maxWidth: "300px" }}
  navHeight: "48px"
  contentMaxWidth: "1200px"
card:
  radius: {{ default: "8px", cut: "0px" }}
  padding: {{ sm: "8px", md: "16px", lg: "24px" }}
  gradient: {{ start: "#fff", mid: "#eee", end: "#ddd" }}
mobile:
  spacing: {{ xs: "4px", sm: "8px", md: "16px", lg: "24px", xl: "32px", xxl: "48px" }}
  typography:
    sizes: {{ xs: "0.75rem", sm: "0.875rem", md: "1rem", lg: "1.125rem", xl: "1.5rem", xxl: "2rem" }}
  layout: {{ headerHeight: "56px", navHeight: "48px" }}
  card:
    padding: {{ sm: "8px", md: "12px", lg: "16px" }}
touchTargets: {{ default: "44px", sm: "32px", lg: "56px" }}
"##
    )
}


const MINIMAL_SOURCES_YAML: &str = "content_sources: {}\n";

fn write_content_config(boot: &TestBootstrap, yaml: &str) {
    fs::write(boot.services_path.join("content/config.yaml"), yaml).expect("write content config");
}

fn write_web_config(boot: &TestBootstrap, yaml: &str) {
    fs::write(boot.services_path.join("web/config.yaml"), yaml).expect("write web config");
}

async fn maybe_db() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

fn tempdir_paths(tmp: &tempfile::TempDir) -> AppPaths {
    let p = tmp.path().to_string_lossy().to_string();
    AppPaths::from_profile(&PathsConfig {
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
async fn sitemap_provider_missing_content_config_is_read_error() {
    let tmp = tempfile::TempDir::new().unwrap();
    let paths = tempdir_paths(&tmp);
    let err = DefaultSitemapProvider::new(&paths)
        .await
        .expect_err("missing content config");
    assert!(
        matches!(err, PublishError::ContentConfigRead { .. }),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn sitemap_provider_malformed_content_config_is_parse_error() {
    let tmp = tempfile::TempDir::new().unwrap();
    let paths = tempdir_paths(&tmp);
    let cfg = paths.system().content_config().to_path_buf();
    fs::create_dir_all(cfg.parent().unwrap()).unwrap();
    fs::write(&cfg, "content_sources: [not, a, map").unwrap();
    let err = DefaultSitemapProvider::new(&paths)
        .await
        .expect_err("malformed content config");
    assert!(
        matches!(err, PublishError::ContentConfigParse { .. }),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn generate_sitemap_malformed_content_config_is_parse_error() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };

    write_content_config(boot, ": not yaml [");
    let err = generate_sitemap(db, &boot.app_paths)
        .await
        .expect_err("malformed content config");
    write_content_config(boot, MINIMAL_SOURCES_YAML);
    assert!(
        matches!(err, PublishError::ContentConfigParse { .. }),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn prerender_content_malformed_content_config_is_parse_error() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };

    write_content_config(boot, "content_sources: [broken");
    let err = prerender_content(db, &boot.app_paths)
        .await
        .expect_err("malformed content config");
    write_content_config(boot, MINIMAL_SOURCES_YAML);
    assert!(
        matches!(err, PublishError::ContentConfigParse { .. }),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn prerender_content_missing_content_config_is_read_error() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };

    let cfg = boot.services_path.join("content/config.yaml");
    let _ = fs::remove_file(&cfg);
    let err = prerender_content(db, &boot.app_paths)
        .await
        .expect_err("missing content config");
    write_content_config(boot, MINIMAL_SOURCES_YAML);
    assert!(
        matches!(err, PublishError::ContentConfigRead { .. }),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn prerender_content_malformed_web_config_is_web_config_error() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };

    write_content_config(boot, MINIMAL_SOURCES_YAML);
    write_web_config(boot, "branding: [not a map");
    let err = prerender_content(db, &boot.app_paths)
        .await
        .expect_err("malformed web config");
    assert!(
        matches!(err, PublishError::WebConfig(_)),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn load_web_config_missing_file_is_io_error() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let cfg = boot.services_path.join("web/config.yaml");
    let _ = fs::remove_file(&cfg);
    let err = load_web_config(&boot.app_paths)
        .await
        .expect_err("missing web config");
    assert!(
        matches!(err, systemprompt_models::WebConfigError::Io { .. }),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn load_web_config_rejects_nonexistent_templates_path() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    write_web_config(
        boot,
        &web_config_yaml_with_templates_path("/nonexistent/templates/dir/zzz"),
    );
    let err = load_web_config(&boot.app_paths)
        .await
        .expect_err("nonexistent templates path must be rejected");
    assert!(
        matches!(err, systemprompt_models::WebConfigError::PathNotFound { ref field, .. } if field == "paths.templates"),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn get_templates_path_prefers_existing_configured_path() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let tmp = tempfile::TempDir::new().unwrap();
    write_web_config(
        boot,
        &web_config_yaml_with_templates_path(&tmp.path().to_string_lossy()),
    );
    let cfg = load_web_config(&boot.app_paths).await.expect("web config");
    assert_eq!(get_templates_path(&cfg, &boot.app_paths), tmp.path());
}

#[tokio::test]
async fn get_templates_path_falls_back_to_web_root_when_unconfigured() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    write_web_config(boot, &web_config_yaml_with_templates_path(""));
    let cfg = load_web_config(&boot.app_paths).await.expect("web config");
    assert_eq!(
        get_templates_path(&cfg, &boot.app_paths),
        boot.app_paths.web().root().join("templates")
    );
}

#[tokio::test]
async fn prerender_content_missing_templates_dir_is_config_error() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };

    write_content_config(boot, MINIMAL_SOURCES_YAML);
    write_web_config(boot, &web_config_yaml_with_templates_path(""));
    let templates_dir = boot.app_paths.web().root().join("templates");
    let _ = fs::remove_dir_all(&templates_dir);
    let err = prerender_content(db, &boot.app_paths)
        .await
        .expect_err("missing templates dir");
    assert!(
        matches!(err, PublishError::Config { ref message, .. } if message.contains("Template directory not found")),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn prerender_content_with_closed_pool_is_fetch_error() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    if fixture_database_url().is_err() {
        return;
    }

    write_web_config(boot, &web_config_yaml_with_templates_path(""));
    write_content_config(
        boot,
        r#"content_sources:
  blog:
    path: "/blog"
    source_id: "closedpoolsrc"
    category_id: "default"
    enabled: true
    sitemap:
      enabled: true
      url_pattern: "/blog/{slug}"
      priority: 0.8
      changefreq: "weekly"
      fetch_from: ""
"#,
    );
    fs::create_dir_all(boot.app_paths.web().root().join("templates")).expect("mkdir templates");

    let err = prerender_content(closed_db_pool().await, &boot.app_paths)
        .await
        .expect_err("closed pool must fail content fetch");
    write_content_config(boot, MINIMAL_SOURCES_YAML);
    assert!(
        matches!(err, PublishError::FetchFailed { .. }),
        "unexpected error: {err:?}"
    );
}
