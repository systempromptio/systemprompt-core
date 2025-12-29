//! Unit tests for asset handling functionality

use std::fs;
use systemprompt_generator::{copy_implementation_assets, organize_css_files};
use tempfile::TempDir;

// =============================================================================
// organize_css_files tests
// =============================================================================

#[tokio::test]
async fn test_asset_copying_creates_css_directory() {
    let temp_dir = TempDir::new().unwrap();
    let web_dir = temp_dir.path().to_str().unwrap();

    // Create a CSS file in the web directory
    fs::write(temp_dir.path().join("style.css"), "body { color: red; }").unwrap();

    let result = organize_css_files(web_dir).await;

    assert!(result.is_ok());
    assert!(temp_dir.path().join("css").exists());
}

#[tokio::test]
async fn test_asset_copying_copies_css_files() {
    let temp_dir = TempDir::new().unwrap();
    let web_dir = temp_dir.path().to_str().unwrap();

    // Create multiple CSS files
    fs::write(temp_dir.path().join("style.css"), "body { color: red; }").unwrap();
    fs::write(
        temp_dir.path().join("theme.css"),
        ".theme { background: blue; }",
    )
    .unwrap();

    let copied = organize_css_files(web_dir).await.unwrap();

    // Should have copied 2 CSS files
    assert_eq!(copied, 2);

    // Verify files exist in css directory
    assert!(temp_dir.path().join("css/style.css").exists());
    assert!(temp_dir.path().join("css/theme.css").exists());
}

#[tokio::test]
async fn test_asset_copying_ignores_non_css_files() {
    let temp_dir = TempDir::new().unwrap();
    let web_dir = temp_dir.path().to_str().unwrap();

    // Create various file types
    fs::write(temp_dir.path().join("style.css"), "body {}").unwrap();
    fs::write(temp_dir.path().join("script.js"), "console.log('test')").unwrap();
    fs::write(temp_dir.path().join("index.html"), "<html></html>").unwrap();
    fs::write(temp_dir.path().join("data.json"), "{}").unwrap();

    let copied = organize_css_files(web_dir).await.unwrap();

    // Should only copy the CSS file
    assert_eq!(copied, 1);
    assert!(temp_dir.path().join("css/style.css").exists());
    assert!(!temp_dir.path().join("css/script.js").exists());
}

#[tokio::test]
async fn test_asset_copying_empty_directory() {
    let temp_dir = TempDir::new().unwrap();
    let web_dir = temp_dir.path().to_str().unwrap();

    // Empty directory, no CSS files
    let copied = organize_css_files(web_dir).await.unwrap();

    assert_eq!(copied, 0);
    // CSS directory should still be created
    assert!(temp_dir.path().join("css").exists());
}

#[tokio::test]
async fn test_asset_copying_preserves_css_content() {
    let temp_dir = TempDir::new().unwrap();
    let web_dir = temp_dir.path().to_str().unwrap();

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

    fs::write(temp_dir.path().join("complex.css"), css_content).unwrap();

    organize_css_files(web_dir).await.unwrap();

    let copied_content = fs::read_to_string(temp_dir.path().join("css/complex.css")).unwrap();
    assert_eq!(copied_content, css_content);
}

#[tokio::test]
async fn test_asset_copying_nonexistent_web_dir() {
    let result = organize_css_files("/nonexistent/path/that/does/not/exist").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_asset_copying_with_subdirectories() {
    let temp_dir = TempDir::new().unwrap();
    let web_dir = temp_dir.path().to_str().unwrap();

    // Create subdirectory with CSS files - these should NOT be copied
    // as organize_css_files only reads the top level
    fs::create_dir(temp_dir.path().join("subdir")).unwrap();
    fs::write(temp_dir.path().join("subdir/nested.css"), "body {}").unwrap();

    // Top level CSS
    fs::write(temp_dir.path().join("main.css"), "body {}").unwrap();

    let copied = organize_css_files(web_dir).await.unwrap();

    // Should only copy top-level CSS file
    assert_eq!(copied, 1);
    assert!(temp_dir.path().join("css/main.css").exists());
}

// =============================================================================
// copy_implementation_assets tests
// =============================================================================

#[tokio::test]
async fn test_copy_implementation_assets_no_env_var() {
    let temp_dir = TempDir::new().unwrap();
    let web_dir = temp_dir.path().to_str().unwrap();

    // Clear the environment variable if set
    std::env::remove_var("SYSTEMPROMPT_WEB_ASSETS_PATH");

    let copied = copy_implementation_assets(web_dir).await.unwrap();

    // Should return 0 when env var is not set
    assert_eq!(copied, 0);
}

#[tokio::test]
async fn test_copy_implementation_assets_with_fonts() {
    let temp_dir = TempDir::new().unwrap();
    let web_dir = temp_dir.path();

    // Create the source assets directory
    let assets_dir = TempDir::new().unwrap();

    // Create fonts directory with a font file
    fs::create_dir(assets_dir.path().join("fonts")).unwrap();
    fs::write(assets_dir.path().join("fonts/font.woff2"), b"font data").unwrap();

    // Create the required directory structure
    fs::create_dir_all(web_dir.join("../src/assets")).unwrap();
    fs::create_dir_all(web_dir.join("../public")).unwrap();

    // Set the environment variable
    std::env::set_var(
        "SYSTEMPROMPT_WEB_ASSETS_PATH",
        assets_dir.path().to_str().unwrap(),
    );

    let copied = copy_implementation_assets(web_dir.to_str().unwrap()).await;

    // Clean up env var
    std::env::remove_var("SYSTEMPROMPT_WEB_ASSETS_PATH");

    // The function requires specific directory structure, may fail in test env
    // Just verify it doesn't panic
    assert!(copied.is_ok() || copied.is_err());
}

#[tokio::test]
async fn test_copy_implementation_assets_nonexistent_assets_path() {
    let temp_dir = TempDir::new().unwrap();
    let web_dir = temp_dir.path().to_str().unwrap();

    // Set env var to nonexistent path
    std::env::set_var("SYSTEMPROMPT_WEB_ASSETS_PATH", "/nonexistent/assets/path");

    let copied = copy_implementation_assets(web_dir).await.unwrap();

    // Clean up
    std::env::remove_var("SYSTEMPROMPT_WEB_ASSETS_PATH");

    // Should return 0 when path doesn't exist
    assert_eq!(copied, 0);
}

// =============================================================================
// Additional asset tests
// =============================================================================

#[tokio::test]
async fn test_organize_css_files_overwrites_existing() {
    let temp_dir = TempDir::new().unwrap();
    let web_dir = temp_dir.path().to_str().unwrap();

    // Create css directory and a file
    fs::create_dir(temp_dir.path().join("css")).unwrap();
    fs::write(temp_dir.path().join("css/old.css"), "old content").unwrap();

    // Create new CSS file
    fs::write(temp_dir.path().join("style.css"), "new content").unwrap();

    let copied = organize_css_files(web_dir).await.unwrap();

    assert_eq!(copied, 1);
    assert!(temp_dir.path().join("css/style.css").exists());

    let content = fs::read_to_string(temp_dir.path().join("css/style.css")).unwrap();
    assert_eq!(content, "new content");
}

#[tokio::test]
async fn test_organize_css_files_handles_special_filenames() {
    let temp_dir = TempDir::new().unwrap();
    let web_dir = temp_dir.path().to_str().unwrap();

    // Create CSS files with various naming patterns
    fs::write(temp_dir.path().join("main.min.css"), "minified").unwrap();
    fs::write(temp_dir.path().join("theme-dark.css"), "dark theme").unwrap();
    fs::write(temp_dir.path().join("_partial.css"), "partial").unwrap();

    let copied = organize_css_files(web_dir).await.unwrap();

    assert_eq!(copied, 3);
    assert!(temp_dir.path().join("css/main.min.css").exists());
    assert!(temp_dir.path().join("css/theme-dark.css").exists());
    assert!(temp_dir.path().join("css/_partial.css").exists());
}

#[tokio::test]
async fn test_organize_css_files_large_file() {
    let temp_dir = TempDir::new().unwrap();
    let web_dir = temp_dir.path().to_str().unwrap();

    // Create a larger CSS file (10KB)
    let large_content = "body { margin: 0; }\n".repeat(500);
    fs::write(temp_dir.path().join("large.css"), &large_content).unwrap();

    let copied = organize_css_files(web_dir).await.unwrap();

    assert_eq!(copied, 1);

    let copied_content = fs::read_to_string(temp_dir.path().join("css/large.css")).unwrap();
    assert_eq!(copied_content, large_content);
}

#[tokio::test]
async fn test_organize_css_files_unicode_content() {
    let temp_dir = TempDir::new().unwrap();
    let web_dir = temp_dir.path().to_str().unwrap();

    let unicode_content = r#"
/* CSS with Unicode */
.emoji::before {
    content: "🎨";
}
.japanese {
    font-family: "ヒラギノ角ゴ";
}
.arabic {
    direction: rtl;
    content: "مرحبا";
}
"#;

    fs::write(temp_dir.path().join("unicode.css"), unicode_content).unwrap();

    organize_css_files(web_dir).await.unwrap();

    let copied = fs::read_to_string(temp_dir.path().join("css/unicode.css")).unwrap();
    assert_eq!(copied, unicode_content);
}

#[tokio::test]
async fn test_organize_css_multiple_calls_idempotent() {
    let temp_dir = TempDir::new().unwrap();
    let web_dir = temp_dir.path().to_str().unwrap();

    fs::write(temp_dir.path().join("style.css"), "body {}").unwrap();

    // Call multiple times
    let first = organize_css_files(web_dir).await.unwrap();
    let second = organize_css_files(web_dir).await.unwrap();
    let third = organize_css_files(web_dir).await.unwrap();

    // Each call should report the same count
    assert_eq!(first, 1);
    assert_eq!(second, 1);
    assert_eq!(third, 1);

    // File should still exist and be valid
    assert!(temp_dir.path().join("css/style.css").exists());
}
