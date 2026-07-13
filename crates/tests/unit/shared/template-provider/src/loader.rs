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
        result.unwrap_err();
    }

    #[tokio::test]
    async fn load_directory_returns_error() {
        let loader = EmbeddedLoader;
        let source = TemplateSource::Directory(PathBuf::from("templates"));
        let result = loader.load(&source).await;
        result.unwrap_err();
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
    fn is_debug() {
        let loader = EmbeddedLoader;
        let debug = format!("{:?}", loader);
        assert!(debug.contains("EmbeddedLoader"));
    }
}

mod filesystem_loader_tests {
    use super::*;
    use systemprompt_template_provider::TemplateLoaderError;
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
        result.unwrap_err();
    }

    #[tokio::test]
    async fn load_directory_source_fails() {
        let loader = test_loader(vec![]);
        let source = TemplateSource::Directory(PathBuf::from("dir"));
        let result = loader.load(&source).await;
        result.unwrap_err();
    }

    #[tokio::test]
    async fn load_with_traversal_fails() {
        let dir = TempDir::new().unwrap();
        let loader = FileSystemLoader::with_path(dir.path());
        let source = TemplateSource::File(PathBuf::from("../../../etc/passwd"));
        let result = loader.load(&source).await;
        result.unwrap_err();
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
        result.unwrap_err();
    }

    #[tokio::test]
    async fn load_directory_with_no_base_paths_fails() {
        let loader = test_loader(vec![]);
        let result = loader
            .load_directory(PathBuf::from("templates").as_path())
            .await;
        result.unwrap_err();
    }

    #[tokio::test]
    async fn load_directory_with_traversal_fails() {
        let dir = TempDir::new().unwrap();
        let loader = FileSystemLoader::with_path(dir.path());
        let result = loader
            .load_directory(PathBuf::from("../../../etc").as_path())
            .await;
        result.unwrap_err();
    }

    #[tokio::test]
    async fn load_directory_loads_html_files() {
        let dir = TempDir::new().unwrap();
        let templates_dir = dir.path().join("templates");
        fs::create_dir(&templates_dir).await.unwrap();
        fs::write(templates_dir.join("page.html"), "<h1>Page</h1>")
            .await
            .unwrap();
        fs::write(templates_dir.join("card.html"), "<div>Card</div>")
            .await
            .unwrap();
        fs::write(templates_dir.join("readme.txt"), "Not a template")
            .await
            .unwrap();

        let loader = FileSystemLoader::with_path(dir.path());
        let result = loader
            .load_directory(PathBuf::from("templates").as_path())
            .await
            .unwrap();

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
        fs::write(templates_dir.join("readme.txt"), "Text file")
            .await
            .unwrap();

        let loader = FileSystemLoader::with_path(dir.path());
        let result = loader
            .load_directory(PathBuf::from("templates").as_path())
            .await
            .unwrap();

        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn load_from_multiple_base_paths() {
        let dir1 = TempDir::new().unwrap();
        let dir2 = TempDir::new().unwrap();
        fs::write(dir2.path().join("found.html"), "Found!")
            .await
            .unwrap();

        let loader = FileSystemLoader::with_path(dir1.path()).add_path(dir2.path());
        let source = TemplateSource::File(PathBuf::from("found.html"));
        let result = loader.load(&source).await.unwrap();
        assert_eq!(result, "Found!");
    }

    #[tokio::test]
    async fn load_absolute_file_within_base_succeeds() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("abs.html");
        fs::write(&file_path, "Absolute!").await.unwrap();

        let loader = FileSystemLoader::with_path(dir.path());
        let canonical = fs::canonicalize(&file_path).await.unwrap();
        let source = TemplateSource::File(canonical);
        let result = loader.load(&source).await.unwrap();
        assert_eq!(result, "Absolute!");
    }

    #[tokio::test]
    async fn load_absolute_file_outside_all_bases_fails() {
        let base = TempDir::new().unwrap();
        let other = TempDir::new().unwrap();
        let file_path = other.path().join("outside.html");
        fs::write(&file_path, "Outside!").await.unwrap();

        let loader = FileSystemLoader::with_path(base.path());
        let canonical = fs::canonicalize(&file_path).await.unwrap();
        let source = TemplateSource::File(canonical);
        let result = loader.load(&source).await;
        result.unwrap_err();
    }

    #[tokio::test]
    async fn load_absolute_file_skips_nonexistent_and_mismatched_bases() {
        let real_base = TempDir::new().unwrap();
        let other = TempDir::new().unwrap();
        let file_path = real_base.path().join("target.html");
        fs::write(&file_path, "Target!").await.unwrap();

        let missing = real_base.path().join("does-not-exist");

        let loader = FileSystemLoader::with_path(missing)
            .add_path(other.path())
            .add_path(real_base.path());

        let canonical = fs::canonicalize(&file_path).await.unwrap();
        let source = TemplateSource::File(canonical);
        let result = loader.load(&source).await.unwrap();
        assert_eq!(result, "Target!");
    }

    #[tokio::test]
    async fn load_directory_absolute_within_base_succeeds() {
        let dir = TempDir::new().unwrap();
        let templates_dir = dir.path().join("templates");
        fs::create_dir(&templates_dir).await.unwrap();
        fs::write(templates_dir.join("page.html"), "<h1>Page</h1>")
            .await
            .unwrap();

        let loader = FileSystemLoader::with_path(dir.path());
        let canonical = fs::canonicalize(&templates_dir).await.unwrap();
        let result = loader.load_directory(canonical.as_path()).await.unwrap();
        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn load_directory_absolute_outside_base_fails() {
        let base = TempDir::new().unwrap();
        let other = TempDir::new().unwrap();

        let loader = FileSystemLoader::with_path(base.path());
        let canonical = fs::canonicalize(other.path()).await.unwrap();
        let result = loader.load_directory(canonical.as_path()).await;
        result.unwrap_err();
    }

    #[tokio::test]
    async fn load_directory_nonexistent_relative_fails() {
        let dir = TempDir::new().unwrap();
        let loader = FileSystemLoader::with_path(dir.path());
        let result = loader
            .load_directory(PathBuf::from("missing").as_path())
            .await;
        result.unwrap_err();
    }

    #[tokio::test]
    async fn load_absolute_nonexistent_file_surfaces_io_error() {
        let dir = TempDir::new().unwrap();
        let loader = FileSystemLoader::with_path(dir.path());
        let absent = dir.path().join("never-written.html");
        let source = TemplateSource::File(absent);
        let result = loader.load(&source).await;
        result.unwrap_err();
    }

    #[tokio::test]
    async fn load_relative_path_pointing_at_directory_surfaces_io_error() {
        let dir = TempDir::new().unwrap();
        let inner = dir.path().join("a-directory");
        fs::create_dir(&inner).await.unwrap();

        let loader = FileSystemLoader::with_path(dir.path());
        let source = TemplateSource::File(PathBuf::from("a-directory"));
        let result = loader.load(&source).await;
        result.unwrap_err();
    }

    #[tokio::test]
    async fn load_directory_target_resolving_to_file_surfaces_io_error() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("not-a-dir.dat"), "payload")
            .await
            .unwrap();

        let loader = FileSystemLoader::with_path(dir.path());
        let result = loader
            .load_directory(PathBuf::from("not-a-dir.dat").as_path())
            .await;
        result.unwrap_err();
    }

    #[tokio::test]
    async fn load_absolute_file_with_unresolvable_base_surfaces_base_io_error() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("t.html");
        fs::write(&file_path, "content").await.unwrap();
        let blocker = dir.path().join("blocker.txt");
        fs::write(&blocker, "not a dir").await.unwrap();
        let bad_base = blocker.join("nested");

        let loader = FileSystemLoader::with_path(&bad_base);
        let canonical = fs::canonicalize(&file_path).await.unwrap();
        let err = loader
            .load(&TemplateSource::File(canonical))
            .await
            .unwrap_err();
        assert!(
            matches!(err, TemplateLoaderError::Io { ref path, .. } if *path == bad_base),
            "{err:?}"
        );
    }

    // chdir is safe here: nextest runs each test in its own process.
    #[tokio::test]
    async fn load_relative_file_with_empty_base_surfaces_base_io_error() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("t.html"), "content").await.unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        let loader = FileSystemLoader::with_path(PathBuf::new());
        let err = loader
            .load(&TemplateSource::File(PathBuf::from("t.html")))
            .await
            .unwrap_err();
        assert!(
            matches!(err, TemplateLoaderError::Io { ref path, .. } if *path == PathBuf::new()),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn load_relative_file_through_escaping_symlink_fails_outside_base() {
        let base = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let target = outside.path().join("t.html");
        fs::write(&target, "outside").await.unwrap();
        let link = base.path().join("esc.html");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let loader = FileSystemLoader::with_path(base.path());
        let err = loader
            .load(&TemplateSource::File(PathBuf::from("esc.html")))
            .await
            .unwrap_err();
        assert!(
            matches!(err, TemplateLoaderError::OutsideBasePath(ref p) if *p == link),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn load_relative_file_through_file_component_surfaces_io_error() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("blocker.txt"), "file").await.unwrap();

        let loader = FileSystemLoader::with_path(dir.path());
        let expected = dir.path().join("blocker.txt/t.html");
        let err = loader
            .load(&TemplateSource::File(PathBuf::from("blocker.txt/t.html")))
            .await
            .unwrap_err();
        assert!(
            matches!(err, TemplateLoaderError::Io { ref path, .. } if *path == expected),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn load_absolute_directory_as_file_surfaces_read_io_error() {
        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).await.unwrap();

        let loader = FileSystemLoader::with_path(dir.path());
        let canonical = fs::canonicalize(&sub).await.unwrap();
        let err = loader
            .load(&TemplateSource::File(canonical.clone()))
            .await
            .unwrap_err();
        assert!(
            matches!(err, TemplateLoaderError::Io { ref path, .. } if *path == canonical),
            "{err:?}"
        );
    }

    // chdir is safe here: nextest runs each test in its own process.
    #[tokio::test]
    async fn load_directory_with_empty_base_surfaces_base_io_error() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("sub")).await.unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        let loader = FileSystemLoader::with_path(PathBuf::new());
        let err = loader
            .load_directory(PathBuf::from("sub").as_path())
            .await
            .unwrap_err();
        assert!(
            matches!(err, TemplateLoaderError::Io { ref path, .. } if *path == PathBuf::new()),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn load_directory_through_escaping_symlink_fails_outside_base() {
        let base = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let link = base.path().join("tpls");
        std::os::unix::fs::symlink(outside.path(), &link).unwrap();

        let loader = FileSystemLoader::with_path(base.path());
        let err = loader
            .load_directory(PathBuf::from("tpls").as_path())
            .await
            .unwrap_err();
        assert!(
            matches!(err, TemplateLoaderError::OutsideBasePath(ref p) if *p == link),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn load_directory_candidate_through_file_component_surfaces_io_error() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("blocker.txt"), "file").await.unwrap();

        let loader = FileSystemLoader::with_path(dir.path());
        let expected = dir.path().join("blocker.txt/sub");
        let err = loader
            .load_directory(PathBuf::from("blocker.txt/sub").as_path())
            .await
            .unwrap_err();
        assert!(
            matches!(err, TemplateLoaderError::Io { ref path, .. } if *path == expected),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn load_directory_non_utf8_stem_fails_invalid_encoding() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("templates");
        fs::create_dir(&sub).await.unwrap();
        let bad_name = sub.join(OsStr::from_bytes(b"\xff\xfe.html"));
        fs::write(&bad_name, "<p>bad</p>").await.unwrap();

        let loader = FileSystemLoader::with_path(dir.path());
        let err = loader
            .load_directory(PathBuf::from("templates").as_path())
            .await
            .unwrap_err();
        assert!(
            matches!(err, TemplateLoaderError::InvalidEncoding(ref p) if *p == bad_name),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn load_directory_unreadable_html_surfaces_io_error() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("templates");
        fs::create_dir(&sub).await.unwrap();
        let locked = sub.join("locked.html");
        fs::write(&locked, "<p>secret</p>").await.unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0)).unwrap();

        let loader = FileSystemLoader::with_path(dir.path());
        let err = loader
            .load_directory(PathBuf::from("templates").as_path())
            .await
            .unwrap_err();
        assert!(
            matches!(err, TemplateLoaderError::Io { ref path, .. } if *path == locked),
            "{err:?}"
        );
    }
}

mod default_method_tests {
    use super::*;
    use std::path::Path;

    struct MinimalLoader;

    #[async_trait::async_trait]
    impl TemplateLoader for MinimalLoader {
        async fn load(
            &self,
            source: &TemplateSource,
        ) -> Result<String, systemprompt_template_provider::TemplateLoaderError> {
            match source {
                TemplateSource::Embedded(content) => Ok((*content).to_owned()),
                _ => Err(systemprompt_template_provider::TemplateLoaderError::EmbeddedOnly),
            }
        }

        fn can_load(&self, source: &TemplateSource) -> bool {
            matches!(source, TemplateSource::Embedded(_))
        }
    }

    #[tokio::test]
    async fn default_load_directory_is_unsupported() {
        let loader = MinimalLoader;
        let result = loader.load_directory(Path::new("anything")).await;
        result.unwrap_err();
    }

    #[tokio::test]
    async fn minimal_loader_load_and_can_load() {
        let loader = MinimalLoader;
        assert_eq!(
            loader.load(&TemplateSource::Embedded("x")).await.unwrap(),
            "x"
        );
        assert!(loader.can_load(&TemplateSource::Embedded("x")));
        assert!(!loader.can_load(&TemplateSource::File(PathBuf::from("a.html"))));
    }
}
