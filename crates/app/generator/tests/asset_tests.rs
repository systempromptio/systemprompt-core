//! Unit tests for asset handling functionality

use std::fs;
use systemprompt_generator::organize_dist_assets;
use tempfile::TempDir;

#[tokio::test]
async fn test_organize_dist_assets_creates_directories() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(temp_dir.path().join("style.css"), "body { color: red; }").unwrap();
    fs::write(temp_dir.path().join("app.js"), "console.log('test')").unwrap();

    let result = organize_dist_assets(temp_dir.path()).await;

    assert!(result.is_ok());
    assert!(temp_dir.path().join("css").exists());
    assert!(temp_dir.path().join("js").exists());
}

#[tokio::test]
async fn test_organize_dist_assets_copies_files() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(temp_dir.path().join("style.css"), "body { color: red; }").unwrap();
    fs::write(
        temp_dir.path().join("theme.css"),
        ".theme { background: blue; }",
    )
    .unwrap();
    fs::write(temp_dir.path().join("app.js"), "console.log('app')").unwrap();
    fs::write(temp_dir.path().join("utils.js"), "export {}").unwrap();

    let (css_count, js_count) = organize_dist_assets(temp_dir.path()).await.unwrap();

    assert_eq!(css_count, 2);
    assert_eq!(js_count, 2);

    assert!(temp_dir.path().join("css/style.css").exists());
    assert!(temp_dir.path().join("css/theme.css").exists());
    assert!(temp_dir.path().join("js/app.js").exists());
    assert!(temp_dir.path().join("js/utils.js").exists());
}

#[tokio::test]
async fn test_organize_dist_assets_ignores_other_files() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(temp_dir.path().join("style.css"), "body {}").unwrap();
    fs::write(temp_dir.path().join("script.js"), "console.log('test')").unwrap();
    fs::write(temp_dir.path().join("index.html"), "<html></html>").unwrap();
    fs::write(temp_dir.path().join("data.json"), "{}").unwrap();

    let (css_count, js_count) = organize_dist_assets(temp_dir.path()).await.unwrap();

    assert_eq!(css_count, 1);
    assert_eq!(js_count, 1);
    assert!(temp_dir.path().join("css/style.css").exists());
    assert!(temp_dir.path().join("js/script.js").exists());
    assert!(!temp_dir.path().join("css/index.html").exists());
    assert!(!temp_dir.path().join("js/data.json").exists());
}

#[tokio::test]
async fn test_organize_dist_assets_empty_directory() {
    let temp_dir = TempDir::new().unwrap();

    let (css_count, js_count) = organize_dist_assets(temp_dir.path()).await.unwrap();

    assert_eq!(css_count, 0);
    assert_eq!(js_count, 0);
    assert!(temp_dir.path().join("css").exists());
    assert!(temp_dir.path().join("js").exists());
}

#[tokio::test]
async fn test_organize_dist_assets_preserves_content() {
    let temp_dir = TempDir::new().unwrap();

    let css_content = r#"
/* Complex CSS with various features */
:root {
    --primary-color: #3498db;
    --secondary-color: #2ecc71;
}

body {
    font-family: 'Helvetica Neue', sans-serif;
    line-height: 1.6;
}

@media (max-width: 768px) {
    .container {
        padding: 1rem;
    }
}
"#;

    let js_content = r#"
// Complex JS
const config = {
    api: '/api/v1',
    timeout: 5000
};

export default config;
"#;

    fs::write(temp_dir.path().join("complex.css"), css_content).unwrap();
    fs::write(temp_dir.path().join("config.js"), js_content).unwrap();

    organize_dist_assets(temp_dir.path()).await.unwrap();

    let copied_css = fs::read_to_string(temp_dir.path().join("css/complex.css")).unwrap();
    let copied_js = fs::read_to_string(temp_dir.path().join("js/config.js")).unwrap();
    assert_eq!(copied_css, css_content);
    assert_eq!(copied_js, js_content);
}

#[tokio::test]
async fn test_organize_dist_assets_nonexistent_dir() {
    let result = organize_dist_assets(std::path::Path::new(
        "/nonexistent/path/that/does/not/exist",
    ))
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_organize_dist_assets_with_subdirectories() {
    let temp_dir = TempDir::new().unwrap();

    fs::create_dir(temp_dir.path().join("subdir")).unwrap();
    fs::write(temp_dir.path().join("subdir/nested.css"), "body {}").unwrap();
    fs::write(temp_dir.path().join("subdir/nested.js"), "export {}").unwrap();

    fs::write(temp_dir.path().join("main.css"), "body {}").unwrap();
    fs::write(temp_dir.path().join("main.js"), "console.log('main')").unwrap();

    let (css_count, js_count) = organize_dist_assets(temp_dir.path()).await.unwrap();

    assert_eq!(css_count, 1);
    assert_eq!(js_count, 1);
    assert!(temp_dir.path().join("css/main.css").exists());
    assert!(temp_dir.path().join("js/main.js").exists());
}

#[tokio::test]
async fn test_organize_dist_assets_overwrites_existing() {
    let temp_dir = TempDir::new().unwrap();

    fs::create_dir(temp_dir.path().join("css")).unwrap();
    fs::create_dir(temp_dir.path().join("js")).unwrap();
    fs::write(temp_dir.path().join("css/old.css"), "old content").unwrap();
    fs::write(temp_dir.path().join("js/old.js"), "old content").unwrap();

    fs::write(temp_dir.path().join("style.css"), "new css content").unwrap();
    fs::write(temp_dir.path().join("app.js"), "new js content").unwrap();

    let (css_count, js_count) = organize_dist_assets(temp_dir.path()).await.unwrap();

    assert_eq!(css_count, 1);
    assert_eq!(js_count, 1);
    assert!(temp_dir.path().join("css/style.css").exists());
    assert!(temp_dir.path().join("js/app.js").exists());

    let css_content = fs::read_to_string(temp_dir.path().join("css/style.css")).unwrap();
    let js_content = fs::read_to_string(temp_dir.path().join("js/app.js")).unwrap();
    assert_eq!(css_content, "new css content");
    assert_eq!(js_content, "new js content");
}

#[tokio::test]
async fn test_organize_dist_assets_handles_special_filenames() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(temp_dir.path().join("main.min.css"), "minified").unwrap();
    fs::write(temp_dir.path().join("theme-dark.css"), "dark theme").unwrap();
    fs::write(temp_dir.path().join("_partial.css"), "partial").unwrap();
    fs::write(temp_dir.path().join("app.bundle.js"), "bundled").unwrap();
    fs::write(temp_dir.path().join("vendor-chunk.js"), "vendor").unwrap();

    let (css_count, js_count) = organize_dist_assets(temp_dir.path()).await.unwrap();

    assert_eq!(css_count, 3);
    assert_eq!(js_count, 2);
    assert!(temp_dir.path().join("css/main.min.css").exists());
    assert!(temp_dir.path().join("css/theme-dark.css").exists());
    assert!(temp_dir.path().join("css/_partial.css").exists());
    assert!(temp_dir.path().join("js/app.bundle.js").exists());
    assert!(temp_dir.path().join("js/vendor-chunk.js").exists());
}

#[tokio::test]
async fn test_organize_dist_assets_large_files() {
    let temp_dir = TempDir::new().unwrap();

    let large_css = "body { margin: 0; }\n".repeat(500);
    let large_js = "console.log('test');\n".repeat(500);
    fs::write(temp_dir.path().join("large.css"), &large_css).unwrap();
    fs::write(temp_dir.path().join("large.js"), &large_js).unwrap();

    let (css_count, js_count) = organize_dist_assets(temp_dir.path()).await.unwrap();

    assert_eq!(css_count, 1);
    assert_eq!(js_count, 1);

    let copied_css = fs::read_to_string(temp_dir.path().join("css/large.css")).unwrap();
    let copied_js = fs::read_to_string(temp_dir.path().join("js/large.js")).unwrap();
    assert_eq!(copied_css, large_css);
    assert_eq!(copied_js, large_js);
}

#[tokio::test]
async fn test_organize_dist_assets_unicode_content() {
    let temp_dir = TempDir::new().unwrap();

    let unicode_css = r#"
/* CSS with Unicode */
.emoji::before {
    content: "🎨";
}
.japanese {
    font-family: "ヒラギノ角ゴ";
}
"#;

    let unicode_js = r#"
// JS with Unicode
const greeting = "مرحبا";
const emoji = "🚀";
"#;

    fs::write(temp_dir.path().join("unicode.css"), unicode_css).unwrap();
    fs::write(temp_dir.path().join("unicode.js"), unicode_js).unwrap();

    organize_dist_assets(temp_dir.path()).await.unwrap();

    let copied_css = fs::read_to_string(temp_dir.path().join("css/unicode.css")).unwrap();
    let copied_js = fs::read_to_string(temp_dir.path().join("js/unicode.js")).unwrap();
    assert_eq!(copied_css, unicode_css);
    assert_eq!(copied_js, unicode_js);
}

#[tokio::test]
async fn test_organize_dist_assets_multiple_calls_idempotent() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(temp_dir.path().join("style.css"), "body {}").unwrap();
    fs::write(temp_dir.path().join("app.js"), "console.log('test')").unwrap();

    let first = organize_dist_assets(temp_dir.path()).await.unwrap();
    let second = organize_dist_assets(temp_dir.path()).await.unwrap();
    let third = organize_dist_assets(temp_dir.path()).await.unwrap();

    assert_eq!(first, (1, 1));
    assert_eq!(second, (1, 1));
    assert_eq!(third, (1, 1));

    assert!(temp_dir.path().join("css/style.css").exists());
    assert!(temp_dir.path().join("js/app.js").exists());
}
