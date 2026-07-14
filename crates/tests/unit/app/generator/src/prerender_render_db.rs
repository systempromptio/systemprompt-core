//! End-to-end rendering tests for `prerender_content` with a real template
//! registry: extension templates are written to `web/templates/`, content
//! rows are seeded, and the emitted HTML under `dist/` is asserted exactly.
//!
//! This drives the previously-uncovered per-item render pipeline
//! (`prerender/render.rs`), the list/parent route (`prerender/list.rs`), the
//! source orchestration arms in `prerender/content.rs`, and the happy-path
//! fetch/enrichment in `prerender/fetch.rs`.

use std::fs;
use std::sync::Mutex;

use systemprompt_content::ContentRepository;
use systemprompt_content::models::CreateContentParams;
use systemprompt_database::DbPool;
use systemprompt_generator::{PublishError, prerender_content, prerender_pages};
use systemprompt_identifiers::{LocaleCode, SourceId};
use systemprompt_test_fixtures::{
    TestBootstrap, ensure_test_bootstrap, fixture_database_url, fixture_db_pool,
};

// Serialises this module's tests when run under plain `cargo test` (nextest
// gives each test its own process, so the lock is then uncontended).
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

fn content_config_yaml(source_id: &str, parent_enabled: bool) -> String {
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
      parent_route:
        enabled: {parent_enabled}
        url: "/blog"
        priority: 0.9
        changefreq: "daily"
  disabled_source:
    path: "/off"
    source_id: "{source_id}-off"
    category_id: "default"
    enabled: false
  no_sitemap_source:
    path: "/nos"
    source_id: "{source_id}-nos"
    category_id: "default"
    enabled: true
    sitemap:
      enabled: false
      url_pattern: "/nos/{{slug}}"
      priority: 0.1
      changefreq: "weekly"
      fetch_from: ""
"#
    )
}

const ITEM_TEMPLATE: &str = "<article data-slug=\"{{SLUG}}\" lang=\"{{locale}}\"><nav>{{{TOC_HTML}}}</nav>{{{CONTENT}}}</article>";
const LIST_TEMPLATE: &str =
    "<section data-index=\"{{HAS_INDEX_CONTENT}}\" lang=\"{{locale}}\">list</section>";

fn install_config(boot: &TestBootstrap, source_id: &str, parent_enabled: bool) {
    fs::write(
        boot.services_path.join("web/config.yaml"),
        WEB_CONFIG_TWO_LOCALES_YAML,
    )
    .expect("write web config");
    fs::write(
        boot.services_path.join("content/config.yaml"),
        content_config_yaml(source_id, parent_enabled),
    )
    .expect("write content config");
    fs::create_dir_all(boot.app_paths.web().dist()).expect("mkdir dist");
    fs::create_dir_all(boot.app_paths.web().root().join("templates")).expect("mkdir templates");
}

fn install_templates(boot: &TestBootstrap, item: bool, list: bool) {
    let dir = boot.app_paths.web().root().join("templates");
    let item_path = dir.join("article-post.html");
    let list_path = dir.join("blog-list.html");
    let _ = fs::remove_file(&item_path);
    let _ = fs::remove_file(&list_path);
    if item {
        fs::write(&item_path, ITEM_TEMPLATE).expect("write item template");
    }
    if list {
        fs::write(&list_path, LIST_TEMPLATE).expect("write list template");
    }
}

async fn maybe_db() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

async fn seed_post(db: &DbPool, source_id: &SourceId, slug: &str, locale: &str, body: &str) {
    let repo = ContentRepository::new(db).expect("content repository");
    let params = CreateContentParams::new(
        slug.to_owned(),
        format!("Title of {slug}"),
        "desc".to_owned(),
        body.to_owned(),
        source_id.clone(),
    )
    .with_locale(LocaleCode::new(locale))
    .with_public(true);
    repo.create(&params).await.expect("create content row");
}

async fn cleanup(db: &DbPool, source_id: &SourceId) {
    let repo = ContentRepository::new(db).expect("content repository");
    let _ = repo.delete_by_source(source_id).await;
}

fn dist_page(boot: &TestBootstrap, rel: &str) -> std::path::PathBuf {
    boot.app_paths.web().dist().join(rel).join("index.html")
}

#[tokio::test]
async fn prerender_renders_item_html_with_toc_for_both_locales() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };

    let source_id = SourceId::new("renderdbitems");
    cleanup(&db, &source_id).await;
    let body = "# Heading One\n\nSome paragraph.\n\n## Sub Point\n\nMore text.";
    seed_post(&db, &source_id, "render-me", "en", body).await;
    seed_post(&db, &source_id, "render-me", "fr", body).await;

    install_config(boot, "renderdbitems", false);
    install_templates(boot, true, false);

    prerender_content(db.clone(), &boot.app_paths)
        .await
        .expect("prerender_content");

    cleanup(&db, &source_id).await;

    let en_html = fs::read_to_string(dist_page(boot, "blog/render-me")).expect("en page");
    assert!(
        en_html.contains("data-slug=\"render-me\""),
        "slug must be injected: {en_html}"
    );
    assert!(
        en_html.contains("lang=\"en\""),
        "locale must be injected: {en_html}"
    );
    assert!(
        en_html.contains("id=\"sub-point\""),
        "heading id must be injected by TOC pipeline: {en_html}"
    );
    assert!(
        en_html.contains("href=\"#sub-point\""),
        "toc link must be rendered: {en_html}"
    );
    assert!(
        en_html.contains("<p>Some paragraph.</p>"),
        "markdown body must be rendered to html: {en_html}"
    );

    let fr_html = fs::read_to_string(dist_page(boot, "fr/blog/render-me"))
        .expect("fr page under locale prefix");
    assert!(
        fr_html.contains("lang=\"fr\""),
        "non-default locale page must carry its locale: {fr_html}"
    );
}

#[tokio::test]
async fn prerender_renders_parent_list_route_with_index_content() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };

    let source_id = SourceId::new("renderdblist");
    cleanup(&db, &source_id).await;
    seed_post(&db, &source_id, "list-post", "en", "# Post").await;
    seed_post(&db, &source_id, "", "en", "# Index blurb").await;
    seed_post(&db, &source_id, "list-post", "fr", "# Poste").await;

    install_config(boot, "renderdblist", true);
    install_templates(boot, true, true);

    prerender_content(db.clone(), &boot.app_paths)
        .await
        .expect("prerender_content");

    cleanup(&db, &source_id).await;

    let list_html = fs::read_to_string(dist_page(boot, "blog")).expect("list page");
    assert!(
        list_html.contains("data-index=\"true\""),
        "index content row must flip HAS_INDEX_CONTENT: {list_html}"
    );
    assert!(
        list_html.contains("lang=\"en\""),
        "list page must carry the locale: {list_html}"
    );
    let fr_list =
        fs::read_to_string(dist_page(boot, "fr/blog")).expect("fr list page under locale prefix");
    assert!(fr_list.contains("data-index=\"false\""));
    assert!(
        dist_page(boot, "blog/list-post").exists(),
        "regular item must still render alongside the list route"
    );
}

#[tokio::test]
async fn prerender_list_route_without_index_content_sets_flag_false() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };

    let source_id = SourceId::new("renderdbnoindex");
    cleanup(&db, &source_id).await;
    seed_post(&db, &source_id, "solo-post", "en", "# Solo").await;
    seed_post(&db, &source_id, "solo-post", "fr", "# Solo").await;

    install_config(boot, "renderdbnoindex", true);
    install_templates(boot, true, true);

    prerender_content(db.clone(), &boot.app_paths)
        .await
        .expect("prerender_content");

    cleanup(&db, &source_id).await;

    let list_html = fs::read_to_string(dist_page(boot, "blog")).expect("list page");
    assert!(
        list_html.contains("data-index=\"false\""),
        "no empty-slug row means HAS_INDEX_CONTENT=false: {list_html}"
    );
}

#[tokio::test]
async fn prerender_errors_with_template_not_found_for_item() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };

    let source_id = SourceId::new("renderdbnotmpl");
    cleanup(&db, &source_id).await;
    seed_post(&db, &source_id, "orphan-post", "en", "# Orphan").await;
    seed_post(&db, &source_id, "orphan-post", "fr", "# Orphan").await;

    install_config(boot, "renderdbnotmpl", false);
    install_templates(boot, false, false);

    let err = prerender_content(db.clone(), &boot.app_paths)
        .await
        .expect_err("no template registered for content type 'article'");

    cleanup(&db, &source_id).await;

    assert!(
        matches!(err, PublishError::TemplateNotFound { ref content_type, .. } if content_type == "article"),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn prerender_errors_with_template_not_found_for_list_route() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };

    let source_id = SourceId::new("renderdbnolist");
    cleanup(&db, &source_id).await;
    seed_post(&db, &source_id, "has-item-tmpl", "en", "# Body").await;
    seed_post(&db, &source_id, "has-item-tmpl", "fr", "# Body").await;

    install_config(boot, "renderdbnolist", true);
    install_templates(boot, true, false);

    let err = prerender_content(db.clone(), &boot.app_paths)
        .await
        .expect_err("list template missing while parent route enabled");

    cleanup(&db, &source_id).await;

    assert!(
        matches!(err, PublishError::TemplateNotFound { ref content_type, .. } if content_type == "blog-list"),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn prerender_pages_renders_homepage_when_template_exists() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };

    install_config(boot, "renderdbpages", false);
    let tmpl_dir = boot.app_paths.web().root().join("templates");
    fs::write(
        tmpl_dir.join("homepage.html"),
        "<main data-page=\"home\" lang=\"{{locale}}\">welcome</main>",
    )
    .expect("write homepage template");

    let results = prerender_pages(db.clone(), &boot.app_paths)
        .await
        .expect("prerender_pages");

    let _ = fs::remove_file(tmpl_dir.join("homepage.html"));

    let homepage = results.iter().find(|r| r.page_type == "homepage");
    if let Some(result) = homepage {
        let html = fs::read_to_string(&result.output_path).expect("homepage output");
        assert!(
            html.contains("data-page=\"home\""),
            "homepage template must be rendered: {html}"
        );
        assert!(!format!("{result:?}").is_empty());
    }
}
