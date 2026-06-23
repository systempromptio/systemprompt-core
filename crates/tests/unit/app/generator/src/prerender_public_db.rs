//! Database-backed tests for the public `prerender_content` entry point's
//! handling of the `markdown_content.public` flag.
//!
//! The prerenderer must honor `public = false` the same way the sitemap and
//! navigation already do: non-public rows must never have static HTML emitted
//! to `web/dist/`, and a row that transitions `public -> private` must have its
//! previously-rendered HTML removed. These paths had no coverage — the existing
//! pipeline tests run against an empty content database, so the public/private
//! partition and the stale-output cleanup never executed.
//!
//! The assertions deliberately do not depend on a working template registry
//! (no extension templates are installed here): they pre-create the dist output
//! a prior render would have produced, then verify that the private slug's
//! output is purged while the public slug's output is retained.

use std::fs;
use std::sync::Mutex;

use systemprompt_content::ContentRepository;
use systemprompt_content::models::{CreateContentParams, UpdateContentParams};
use systemprompt_database::DbPool;
use systemprompt_generator::prerender_content;
use systemprompt_identifiers::SourceId;
use systemprompt_test_fixtures::{
    TestBootstrap, ensure_test_bootstrap, fixture_database_url, fixture_db_pool,
};

/// Serialises this module's tests so each one can write the shared
/// `web/config.yaml` + `content/config.yaml` and run the prerenderer without a
/// sibling test in this module overwriting the config mid-flight.
static SERIALIZE: Mutex<()> = Mutex::new(());

const WEB_CONFIG_YAML: &str = r##"
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
  supported_locales: [en]
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
    source_id: "prerenderpubtest"
    category_id: "default"
    enabled: true
    sitemap:
      enabled: true
      url_pattern: "/blog/{slug}"
      priority: 0.8
      changefreq: "weekly"
      fetch_from: ""
      parent_route:
        enabled: false
        url: "/blog"
        priority: 0.9
        changefreq: "daily"
    branding:
      name: "Blog Feed"
      description: "Blog posts"
"##;

const TEST_SOURCE_ID: &str = "prerenderpubtest";

fn install_config(boot: &TestBootstrap) {
    fs::create_dir_all(boot.services_path.join("web")).ok();
    fs::create_dir_all(boot.services_path.join("content")).ok();
    fs::write(boot.services_path.join("web/config.yaml"), WEB_CONFIG_YAML).expect("write web config");
    fs::write(
        boot.services_path.join("content/config.yaml"),
        CONTENT_CONFIG_WITH_SOURCE_YAML,
    )
    .expect("write content config");
    fs::create_dir_all(boot.app_paths.web().dist()).ok();
    fs::create_dir_all(boot.app_paths.web().root().join("templates")).ok();
}

async fn maybe_db() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

/// Path the default-locale prerenderer would write for `slug`, mirroring
/// `write_rendered_page` with `url_pattern = "/blog/{slug}"` and no locale
/// prefix: `dist/blog/{slug}/index.html`.
fn rendered_page_path(boot: &TestBootstrap, slug: &str) -> std::path::PathBuf {
    boot.app_paths
        .web()
        .dist()
        .join("blog")
        .join(slug)
        .join("index.html")
}

fn seed_rendered_page(boot: &TestBootstrap, slug: &str) {
    let path = rendered_page_path(boot, slug);
    fs::create_dir_all(path.parent().expect("parent")).expect("mkdir rendered page dir");
    fs::write(&path, "<html>stale</html>").expect("seed rendered page");
}

async fn clean_source(repo: &ContentRepository, source_id: &SourceId) {
    repo.delete_by_source(source_id)
        .await
        .expect("clean source rows");
}

#[tokio::test]
async fn prerender_excludes_non_public_rows() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };

    let repo = ContentRepository::new(&db).expect("content repository");
    let source_id = SourceId::new(TEST_SOURCE_ID);
    clean_source(&repo, &source_id).await;

    let public_slug = "prerender-public-post";
    let private_slug = "prerender-private-post";

    repo.create(
        &CreateContentParams::new(
            public_slug.to_owned(),
            "Public Post".to_owned(),
            "shown".to_owned(),
            "# shown".to_owned(),
            source_id.clone(),
        )
        .with_public(true),
    )
    .await
    .expect("create public row");
    repo.create(
        &CreateContentParams::new(
            private_slug.to_owned(),
            "Private Post".to_owned(),
            "hidden".to_owned(),
            "# hidden".to_owned(),
            source_id.clone(),
        )
        .with_public(false),
    )
    .await
    .expect("create private row");

    install_config(boot);
    seed_rendered_page(boot, public_slug);
    seed_rendered_page(boot, private_slug);

    // Rendering the public row needs an extension template registry that this
    // harness does not install, so the render itself may error; the public/
    // private partition and the private-slug cleanup run regardless.
    let _ = prerender_content(db.clone(), &boot.app_paths).await;

    clean_source(&repo, &source_id).await;

    assert!(
        !rendered_page_path(boot, private_slug).exists(),
        "non-public slug output was not purged from dist"
    );
    assert!(
        rendered_page_path(boot, public_slug).exists(),
        "public slug output must be retained in dist"
    );
}

#[tokio::test]
async fn prerender_removes_now_private_slug() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };

    let repo = ContentRepository::new(&db).expect("content repository");
    let source_id = SourceId::new(TEST_SOURCE_ID);
    clean_source(&repo, &source_id).await;

    let slug = "prerender-transition-post";
    let created = repo
        .create(
            &CreateContentParams::new(
                slug.to_owned(),
                "Transition Post".to_owned(),
                "was public".to_owned(),
                "# body".to_owned(),
                source_id.clone(),
            )
            .with_public(true),
        )
        .await
        .expect("create public row");

    install_config(boot);
    seed_rendered_page(boot, slug);
    assert!(
        rendered_page_path(boot, slug).exists(),
        "precondition: rendered page exists before transition"
    );

    repo.update(
        &UpdateContentParams::new(
            created.id.clone(),
            "Transition Post".to_owned(),
            "now private".to_owned(),
            "# body".to_owned(),
        )
        .with_version_hash(created.version_hash.clone())
        .with_public(Some(false)),
    )
    .await
    .expect("flip row to private");

    let _ = prerender_content(db.clone(), &boot.app_paths).await;

    clean_source(&repo, &source_id).await;

    assert!(
        !rendered_page_path(boot, slug).exists(),
        "output for slug that transitioned public->private was not removed"
    );
}
