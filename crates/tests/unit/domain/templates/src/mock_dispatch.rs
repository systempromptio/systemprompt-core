use systemprompt_template_provider::{TemplateLoader, TemplateSource};
use systemprompt_templates::TemplateError;

use crate::mocks::MockLoader;

mod error_from_impls {
    use super::*;

    #[test]
    fn io_error_from_std_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let template_err: TemplateError = io_err.into();
        let display = template_err.to_string();
        assert!(display.contains("io error"));
        assert!(matches!(template_err, TemplateError::Io(_)));
    }

    #[test]
    fn io_error_not_found_variant() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let template_err: TemplateError = io_err.into();
        assert!(matches!(template_err, TemplateError::Io(_)));
        assert!(template_err.to_string().contains("io error"));
    }

    #[test]
    fn io_error_other_variant() {
        let io_err = std::io::Error::other("something went wrong");
        let template_err: TemplateError = io_err.into();
        assert!(matches!(template_err, TemplateError::Io(_)));
    }

    #[test]
    fn io_error_source_is_some() {
        use std::error::Error;
        let io_err = std::io::Error::new(std::io::ErrorKind::TimedOut, "timed out");
        let template_err: TemplateError = io_err.into();
        assert!(
            template_err.source().is_some(),
            "Io variant should have a source error"
        );
    }

    #[test]
    fn io_error_display_prefix() {
        let io_err = std::io::Error::new(std::io::ErrorKind::WouldBlock, "blocked");
        let template_err: TemplateError = io_err.into();
        assert!(template_err.to_string().starts_with("io error:"));
    }
}

mod mock_loader_dispatch {
    use super::*;

    #[tokio::test]
    async fn loader_load_embedded_source_returns_content() {
        let loader = MockLoader::new();
        let source = TemplateSource::Embedded("hello embedded world");

        let result = loader.load(&source).await.expect("should load embedded");
        assert_eq!(result, "hello embedded world");
    }

    #[tokio::test]
    async fn loader_load_file_source_formats_path() {
        let loader = MockLoader::new();
        let source = TemplateSource::File(std::path::PathBuf::from("my-template.html"));

        let result = loader.load(&source).await.expect("should load file source");
        assert!(
            result.contains("my-template.html"),
            "file source output should include filename"
        );
    }

    #[tokio::test]
    async fn loader_load_directory_source_formats_path() {
        let loader = MockLoader::new();
        let source = TemplateSource::Directory(std::path::PathBuf::from("/templates/dir"));

        let result = loader
            .load(&source)
            .await
            .expect("should load directory source");
        assert!(
            result.contains("templates") || result.contains("dir"),
            "directory source output should include path components"
        );
    }

    #[tokio::test]
    async fn loader_failing_returns_not_found_error() {
        let loader = MockLoader::failing();
        let source = TemplateSource::Embedded("any content");

        let result = loader.load(&source).await;
        assert!(result.is_err(), "failing loader should return an error");
    }

    #[test]
    fn loader_can_load_returns_true_for_embedded() {
        let loader = MockLoader::new();
        assert!(loader.can_load(&TemplateSource::Embedded("x")));
    }

    #[test]
    fn loader_can_load_returns_true_for_file() {
        let loader = MockLoader::new();
        assert!(loader.can_load(&TemplateSource::File(std::path::PathBuf::from("f.html"))));
    }

    #[test]
    fn loader_can_load_returns_true_for_directory() {
        let loader = MockLoader::new();
        assert!(loader.can_load(&TemplateSource::Directory(std::path::PathBuf::from("d"))));
    }

    #[tokio::test]
    async fn loader_tracks_load_count_across_calls() {
        let loader = MockLoader::new();
        let source = TemplateSource::Embedded("x");

        assert_eq!(loader.load_count(), 0);
        loader.load(&source).await.unwrap();
        assert_eq!(loader.load_count(), 1);
        loader.load(&source).await.unwrap();
        loader.load(&source).await.unwrap();
        assert_eq!(loader.load_count(), 3);
    }

    #[tokio::test]
    async fn failing_loader_still_increments_count() {
        let loader = MockLoader::failing();
        let source = TemplateSource::Embedded("x");

        let _ = loader.load(&source).await;
        assert_eq!(
            loader.load_count(),
            1,
            "failing loader should still increment count"
        );
    }

    #[test]
    fn mock_loader_default_is_non_failing() {
        let loader = MockLoader::default();
        assert_eq!(loader.load_count(), 0);
    }
}

mod mock_provider_dispatch {
    use crate::mocks::MockProvider;
    use systemprompt_template_provider::{TemplateDefinition, TemplateProvider};

    #[test]
    fn provider_id_returns_configured_id() {
        let p = MockProvider::new("my-provider");
        assert_eq!(p.provider_id(), "my-provider");
    }

    #[test]
    fn provider_priority_returns_default_100() {
        let p = MockProvider::new("p");
        assert_eq!(p.priority(), 100);
    }

    #[test]
    fn provider_with_priority_returns_custom_priority() {
        let p = MockProvider::with_priority("p", 42);
        assert_eq!(p.priority(), 42);
    }

    #[test]
    fn provider_templates_returns_configured_templates() {
        let templates = vec![
            TemplateDefinition::embedded("t1", "<p>1</p>"),
            TemplateDefinition::embedded("t2", "<p>2</p>"),
        ];
        let p = MockProvider::with_templates("p", templates);
        assert_eq!(p.templates().len(), 2);
        assert_eq!(p.templates()[0].name, "t1");
        assert_eq!(p.templates()[1].name, "t2");
    }

    #[test]
    fn provider_with_templates_and_priority_combines_both() {
        let templates = vec![TemplateDefinition::embedded("t", "<p>T</p>")];
        let p = MockProvider::with_templates_and_priority("p", 250, templates);
        assert_eq!(p.priority(), 250);
        assert_eq!(p.templates().len(), 1);
    }
}

mod mock_extender_dispatch {
    use crate::mocks::MockExtender;
    use systemprompt_template_provider::TemplateDataExtender;

    #[test]
    fn extender_id_returns_configured_id() {
        let e = MockExtender::new("my-extender");
        assert_eq!(e.extender_id(), "my-extender");
    }

    #[test]
    fn extender_applies_to_empty_by_default() {
        let e = MockExtender::new("e");
        assert!(e.applies_to().is_empty());
    }

    #[test]
    fn extender_with_applies_to_returns_types() {
        let e = MockExtender::with_applies_to("e", vec!["article".to_string(), "blog".to_string()]);
        let applies = e.applies_to();
        assert_eq!(applies.len(), 2);
        assert!(applies.contains(&"article".to_string()));
        assert!(applies.contains(&"blog".to_string()));
    }

    #[test]
    fn extender_with_priority_returns_priority() {
        let e = MockExtender::with_priority("e", 55);
        assert_eq!(e.priority(), 55);
    }

    #[test]
    fn extender_default_priority_is_100() {
        let e = MockExtender::new("e");
        assert_eq!(e.priority(), 100);
    }
}

mod mock_component_dispatch {
    use crate::mocks::MockComponent;
    use systemprompt_template_provider::ComponentRenderer;

    #[test]
    fn component_id_returns_configured_id() {
        let c = MockComponent::new("my-comp", "my_var");
        assert_eq!(c.component_id(), "my-comp");
    }

    #[test]
    fn component_variable_name_returns_configured_name() {
        let c = MockComponent::new("c", "the_variable");
        assert_eq!(c.variable_name(), "the_variable");
    }

    #[test]
    fn component_applies_to_empty_by_default() {
        let c = MockComponent::new("c", "v");
        assert!(c.applies_to().is_empty());
    }

    #[test]
    fn component_with_applies_to_returns_types() {
        let c = MockComponent::with_applies_to(
            "c",
            "v",
            vec!["article".to_string(), "guide".to_string()],
        );
        let applies = c.applies_to();
        assert_eq!(applies.len(), 2);
        assert!(applies.contains(&"article".to_string()));
    }

    #[test]
    fn component_partial_template_none_by_default() {
        let c = MockComponent::new("c", "v");
        assert!(c.partial_template().is_none());
    }
}

mod mock_page_provider_dispatch {
    use crate::mocks::MockPageProvider;
    use systemprompt_template_provider::PageDataProvider;

    #[test]
    fn page_provider_id_returns_configured_id() {
        let pp = MockPageProvider::new("home-data");
        assert_eq!(pp.provider_id(), "home-data");
    }

    #[test]
    fn page_provider_applies_to_pages_empty_by_default() {
        let pp = MockPageProvider::new("pp");
        assert!(pp.applies_to_pages().is_empty());
    }

    #[test]
    fn page_provider_with_applies_to_returns_pages() {
        let pp =
            MockPageProvider::with_applies_to("pp", vec!["home".to_string(), "about".to_string()]);
        let pages = pp.applies_to_pages();
        assert_eq!(pages.len(), 2);
        assert!(pages.contains(&"home".to_string()));
        assert!(pages.contains(&"about".to_string()));
    }

    #[test]
    fn page_provider_with_priority_returns_priority() {
        let pp = MockPageProvider::with_priority("pp", 333);
        assert_eq!(pp.priority(), 333);
    }

    #[test]
    fn page_provider_default_priority_is_100() {
        let pp = MockPageProvider::new("pp");
        assert_eq!(pp.priority(), 100);
    }
}
