use std::path::PathBuf;

use systemprompt_provider_contracts::{ExtendedData, PartialSource, PartialTemplate, RenderedComponent};
use systemprompt_template_provider::{DynTemplateLoader, EmbeddedLoader};

mod partial_template {
    use super::*;

    #[test]
    fn embedded_sets_name_and_source() {
        let pt = PartialTemplate::embedded("nav", "<nav></nav>");
        assert_eq!(pt.name, "nav");
        assert!(matches!(pt.source, PartialSource::Embedded("<nav></nav>")));
    }

    #[test]
    fn file_sets_name_and_source() {
        let pt = PartialTemplate::file("footer", "partials/footer.html");
        assert_eq!(pt.name, "footer");
        assert!(matches!(pt.source, PartialSource::File(_)));
    }

    #[test]
    fn file_path_is_preserved() {
        let pt = PartialTemplate::file("hdr", "partials/header.html");
        if let PartialSource::File(p) = &pt.source {
            assert_eq!(p, &PathBuf::from("partials/header.html"));
        } else {
            panic!("expected File source");
        }
    }

    #[test]
    fn partial_template_is_debug() {
        let pt = PartialTemplate::embedded("x", "y");
        let s = format!("{:?}", pt);
        assert!(s.contains("PartialTemplate"));
    }

    #[test]
    fn partial_template_clones() {
        let pt = PartialTemplate::embedded("nav", "<nav/>");
        let c = pt.clone();
        assert_eq!(c.name, "nav");
    }

    #[test]
    fn partial_source_embedded_is_debug() {
        let s = format!("{:?}", PartialSource::Embedded("hi"));
        assert!(s.contains("Embedded"));
    }

    #[test]
    fn partial_source_file_is_debug() {
        let s = format!("{:?}", PartialSource::File(PathBuf::from("f.html")));
        assert!(s.contains("File"));
    }

    #[test]
    fn partial_source_clones() {
        let src = PartialSource::Embedded("data");
        let c = src.clone();
        assert!(matches!(c, PartialSource::Embedded("data")));
    }
}

mod rendered_component {
    use super::*;

    #[test]
    fn new_sets_both_fields() {
        let rc = RenderedComponent::new("sidebar", "<aside>content</aside>");
        assert_eq!(rc.variable_name, "sidebar");
        assert_eq!(rc.html, "<aside>content</aside>");
    }

    #[test]
    fn is_debug() {
        let rc = RenderedComponent::new("v", "<div/>");
        let s = format!("{:?}", rc);
        assert!(s.contains("RenderedComponent"));
    }

    #[test]
    fn accepts_string_args() {
        let name = String::from("my_var");
        let html = String::from("<p>Hello</p>");
        let rc = RenderedComponent::new(name, html);
        assert_eq!(rc.variable_name, "my_var");
        assert_eq!(rc.html, "<p>Hello</p>");
    }
}

mod extended_data {
    use serde_json::json;

    use super::*;

    #[test]
    fn new_sets_variables_and_default_priority() {
        let vars = json!({"key": "value"});
        let ed = ExtendedData::new(vars.clone());
        assert_eq!(ed.variables, vars);
        assert_eq!(ed.priority, 100);
    }

    #[test]
    fn with_priority_sets_both() {
        let vars = json!({"x": 1});
        let ed = ExtendedData::with_priority(vars.clone(), 50);
        assert_eq!(ed.variables, vars);
        assert_eq!(ed.priority, 50);
    }

    #[test]
    fn with_priority_zero() {
        let ed = ExtendedData::with_priority(json!(null), 0);
        assert_eq!(ed.priority, 0);
    }

    #[test]
    fn with_priority_max() {
        let ed = ExtendedData::with_priority(json!(true), u32::MAX);
        assert_eq!(ed.priority, u32::MAX);
    }

    #[test]
    fn is_debug() {
        let ed = ExtendedData::new(json!({"a": 1}));
        let s = format!("{:?}", ed);
        assert!(s.contains("ExtendedData"));
    }
}

mod dyn_type_aliases {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn dyn_template_loader_is_arc_trait_object() {
        let loader: DynTemplateLoader = Arc::new(EmbeddedLoader);
        assert!(Arc::strong_count(&loader) >= 1);
    }
}

mod embedded_loader_default_load_directory {
    use systemprompt_template_provider::{EmbeddedLoader, TemplateLoader, TemplateLoaderError};

    #[tokio::test]
    async fn returns_directory_loading_unsupported() {
        let loader = EmbeddedLoader;
        let result = loader
            .load_directory(std::path::Path::new("any/path"))
            .await;
        assert!(matches!(
            result.unwrap_err(),
            TemplateLoaderError::DirectoryLoadingUnsupported
        ));
    }
}

mod filesystem_loader_extended {
    use systemprompt_provider_contracts::TemplateSource;
    use systemprompt_template_provider::{FileSystemLoader, TemplateLoader, TemplateLoaderError};
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn load_absolute_embedded_source_works() {
        let loader = FileSystemLoader::default();
        let source = TemplateSource::Embedded("static content");
        let result = loader.load(&source).await.unwrap();
        assert_eq!(result, "static content");
    }

    #[tokio::test]
    async fn load_absolute_file_within_base_path_works() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("abs.html");
        fs::write(&file, "absolute file content").await.unwrap();

        let loader = FileSystemLoader::with_path(dir.path());
        let source = TemplateSource::File(file);
        let result = loader.load(&source).await.unwrap();
        assert_eq!(result, "absolute file content");
    }

    #[tokio::test]
    async fn load_directory_source_returns_directory_not_supported() {
        let loader = FileSystemLoader::default();
        let source = TemplateSource::Directory(std::path::PathBuf::from("dir"));
        let result = loader.load(&source).await;
        assert!(matches!(
            result.unwrap_err(),
            TemplateLoaderError::DirectoryNotSupported(_)
        ));
    }

    #[tokio::test]
    async fn load_dir_nonexistent_relative_dir_returns_not_found() {
        let dir = TempDir::new().unwrap();
        let loader = FileSystemLoader::with_path(dir.path());
        let result = loader
            .load_directory(std::path::Path::new("no_such_dir"))
            .await;
        assert!(matches!(
            result.unwrap_err(),
            TemplateLoaderError::NotFound(_)
        ));
    }

    #[tokio::test]
    async fn load_absolute_directory_absolute_path_works() {
        let dir = TempDir::new().unwrap();
        let subdir = dir.path().join("abs_templates");
        fs::create_dir(&subdir).await.unwrap();
        fs::write(subdir.join("layout.html"), "<html/>")
            .await
            .unwrap();

        let loader = FileSystemLoader::with_path(dir.path());
        let result = loader.load_directory(&subdir).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "layout");
        assert_eq!(result[0].1, "<html/>");
    }

    #[tokio::test]
    async fn load_with_traversal_in_directory_fails() {
        let dir = TempDir::new().unwrap();
        let loader = FileSystemLoader::with_path(dir.path());
        let result = loader
            .load_directory(std::path::Path::new("../escape"))
            .await;
        assert!(matches!(
            result.unwrap_err(),
            TemplateLoaderError::DirectoryTraversal(_)
        ));
    }

    #[tokio::test]
    async fn load_file_relative_not_found_returns_not_found_error() {
        let dir = TempDir::new().unwrap();
        let loader = FileSystemLoader::with_path(dir.path());
        let source = TemplateSource::File(std::path::PathBuf::from("missing.html"));
        let result = loader.load(&source).await;
        assert!(matches!(
            result.unwrap_err(),
            TemplateLoaderError::NotFound(_)
        ));
    }
}
