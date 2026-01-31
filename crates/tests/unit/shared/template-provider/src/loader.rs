//! Tests for template loaders

use std::path::PathBuf;
use systemprompt_provider_contracts::TemplateSource;
use systemprompt_template_provider::{EmbeddedLoader, FileSystemLoader, TemplateLoader};

mod embedded_loader_tests {
    use super::*;

    #[tokio::test]
    async fn load_embedded_content() {
        let loader = EmbeddedLoader;
        let source = TemplateSource::Embedded("Hello World");
        let result = loader.load(&source).await.unwrap();
        assert_eq!(result, "Hello World");
    }

    #[tokio::test]
    async fn load_file_returns_error() {
        let loader = EmbeddedLoader;
        let source = TemplateSource::File(PathBuf::from("test.html"));
        let result = loader.load(&source).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn load_directory_returns_error() {
        let loader = EmbeddedLoader;
        let source = TemplateSource::Directory(PathBuf::from("templates"));
        let result = loader.load(&source).await;
        assert!(result.is_err());
    }

    #[test]
    fn can_load_embedded() {
        let loader = EmbeddedLoader;
        let source = TemplateSource::Embedded("content");
        assert!(loader.can_load(&source));
    }

    #[test]
    fn cannot_load_file() {
        let loader = EmbeddedLoader;
        let source = TemplateSource::File(PathBuf::from("test.html"));
        assert!(!loader.can_load(&source));
    }

    #[test]
    fn cannot_load_directory() {
        let loader = EmbeddedLoader;
        let source = TemplateSource::Directory(PathBuf::from("dir"));
        assert!(!loader.can_load(&source));
    }

    #[test]
    fn is_default() {
        let _loader = EmbeddedLoader::default();
    }

    #[test]
    fn is_clone() {
        let loader = EmbeddedLoader;
        let _cloned = loader;
    }

    #[test]
    fn is_copy() {
        let loader = EmbeddedLoader;
        let copied: EmbeddedLoader = loader;
        let _ = copied;
    }

    #[test]
    fn is_debug() {
        let loader = EmbeddedLoader;
        let debug = format!("{:?}", loader);
        assert!(debug.contains("EmbeddedLoader"));
    }
}

mod filesystem_loader_tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    fn test_loader(paths: Vec<PathBuf>) -> FileSystemLoader {
        FileSystemLoader::new(paths)
    }

    #[test]
    fn new_creates_loader() {
        let paths = vec![PathBuf::from("/templates")];
        let loader = FileSystemLoader::new(paths);
        let debug = format!("{:?}", loader);
        assert!(debug.contains("FileSystemLoader"));
    }

    #[test]
    fn with_path_creates_single_path_loader() {
        let loader = FileSystemLoader::with_path("/templates");
        let debug = format!("{:?}", loader);
        assert!(debug.contains("templates"));
    }

    #[test]
    fn add_path_adds_to_loader() {
        let loader = FileSystemLoader::with_path("/path1").add_path("/path2");
        let debug = format!("{:?}", loader);
        assert!(debug.contains("path1"));
        assert!(debug.contains("path2"));
    }

    #[test]
    fn default_creates_empty_paths() {
        let loader = FileSystemLoader::default();
        let debug = format!("{:?}", loader);
        assert!(debug.contains("FileSystemLoader"));
    }

    #[test]
    fn can_load_embedded() {
        let loader = test_loader(vec![]);
        let source = TemplateSource::Embedded("content");
        assert!(loader.can_load(&source));
    }

    #[test]
    fn can_load_file() {
        let loader = test_loader(vec![]);
        let source = TemplateSource::File(PathBuf::from("test.html"));
        assert!(loader.can_load(&source));
    }

    #[test]
    fn cannot_load_directory() {
        let loader = test_loader(vec![]);
        let source = TemplateSource::Directory(PathBuf::from("templates"));
        assert!(!loader.can_load(&source));
    }

    #[tokio::test]
    async fn load_embedded_content() {
        let loader = test_loader(vec![]);
        let source = TemplateSource::Embedded("embedded content");
        let result = loader.load(&source).await.unwrap();
        assert_eq!(result, "embedded content");
    }

    #[tokio::test]
    async fn load_file_with_no_base_paths_fails() {
        let loader = test_loader(vec![]);
        let source = TemplateSource::File(PathBuf::from("test.html"));
        let result = loader.load(&source).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn load_directory_source_fails() {
        let loader = test_loader(vec![]);
        let source = TemplateSource::Directory(PathBuf::from("dir"));
        let result = loader.load(&source).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn load_with_traversal_fails() {
        let dir = TempDir::new().unwrap();
        let loader = FileSystemLoader::with_path(dir.path());
        let source = TemplateSource::File(PathBuf::from("../../../etc/passwd"));
        let result = loader.load(&source).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn load_existing_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.html");
        fs::write(&file_path, "Hello Template").await.unwrap();

        let loader = FileSystemLoader::with_path(dir.path());
        let source = TemplateSource::File(PathBuf::from("test.html"));
        let result = loader.load(&source).await.unwrap();
        assert_eq!(result, "Hello Template");
    }

    #[tokio::test]
    async fn load_nonexistent_file_fails() {
        let dir = TempDir::new().unwrap();
        let loader = FileSystemLoader::with_path(dir.path());
        let source = TemplateSource::File(PathBuf::from("nonexistent.html"));
        let result = loader.load(&source).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn load_directory_with_no_base_paths_fails() {
        let loader = test_loader(vec![]);
        let result = loader.load_directory(PathBuf::from("templates").as_path()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn load_directory_with_traversal_fails() {
        let dir = TempDir::new().unwrap();
        let loader = FileSystemLoader::with_path(dir.path());
        let result = loader.load_directory(PathBuf::from("../../../etc").as_path()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn load_directory_loads_html_files() {
        let dir = TempDir::new().unwrap();
        let templates_dir = dir.path().join("templates");
        fs::create_dir(&templates_dir).await.unwrap();
        fs::write(templates_dir.join("page.html"), "<h1>Page</h1>").await.unwrap();
        fs::write(templates_dir.join("card.html"), "<div>Card</div>").await.unwrap();
        fs::write(templates_dir.join("readme.txt"), "Not a template").await.unwrap();

        let loader = FileSystemLoader::with_path(dir.path());
        let result = loader.load_directory(PathBuf::from("templates").as_path()).await.unwrap();

        assert_eq!(result.len(), 2);
        let names: Vec<&str> = result.iter().map(|(name, _)| name.as_str()).collect();
        assert!(names.contains(&"page"));
        assert!(names.contains(&"card"));
    }

    #[tokio::test]
    async fn load_directory_returns_empty_for_no_html() {
        let dir = TempDir::new().unwrap();
        let templates_dir = dir.path().join("templates");
        fs::create_dir(&templates_dir).await.unwrap();
        fs::write(templates_dir.join("readme.txt"), "Text file").await.unwrap();

        let loader = FileSystemLoader::with_path(dir.path());
        let result = loader.load_directory(PathBuf::from("templates").as_path()).await.unwrap();

        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn load_from_multiple_base_paths() {
        let dir1 = TempDir::new().unwrap();
        let dir2 = TempDir::new().unwrap();
        fs::write(dir2.path().join("found.html"), "Found!").await.unwrap();

        let loader = FileSystemLoader::with_path(dir1.path()).add_path(dir2.path());
        let source = TemplateSource::File(PathBuf::from("found.html"));
        let result = loader.load(&source).await.unwrap();
        assert_eq!(result, "Found!");
    }
}
