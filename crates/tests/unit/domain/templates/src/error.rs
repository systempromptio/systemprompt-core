use systemprompt_templates::TemplateError;

mod template_error_display_tests {
    use super::*;

    #[test]
    fn not_found_displays_template_name() {
        let error = TemplateError::NotFound("my-template".to_string());

        let display = error.to_string();
        assert!(display.contains("Template not found"));
        assert!(display.contains("my-template"));
    }

    #[test]
    fn load_error_displays_name_and_source() {
        let error = TemplateError::LoadError {
            name: "broken-template".to_string(),
            source: anyhow::anyhow!("file not accessible"),
        };

        let display = error.to_string();
        assert!(display.contains("Failed to load template"));
        assert!(display.contains("broken-template"));
    }

    #[test]
    fn compile_error_displays_name_and_source() {
        let error = TemplateError::CompileError {
            name: "invalid-syntax".to_string(),
            source: anyhow::anyhow!("unexpected token"),
        };

        let display = error.to_string();
        assert!(display.contains("Failed to compile template"));
        assert!(display.contains("invalid-syntax"));
    }

    #[test]
    fn render_error_displays_name_and_source() {
        let error = TemplateError::RenderError {
            name: "render-fail".to_string(),
            source: anyhow::anyhow!("missing variable"),
        };

        let display = error.to_string();
        assert!(display.contains("Failed to render template"));
        assert!(display.contains("render-fail"));
    }

    #[test]
    fn no_loader_displays_template_name() {
        let error = TemplateError::NoLoader("orphan-template".to_string());

        let display = error.to_string();
        assert!(display.contains("No loader available"));
        assert!(display.contains("orphan-template"));
    }

    #[test]
    fn not_initialized_displays_message() {
        let error = TemplateError::NotInitialized;

        let display = error.to_string();
        assert!(display.contains("not initialized"));
    }
}

mod template_error_debug_tests {
    use super::*;

    #[test]
    fn not_found_debug() {
        let error = TemplateError::NotFound("test".to_string());

        let debug = format!("{:?}", error);
        assert!(debug.contains("NotFound"));
    }

    #[test]
    fn load_error_debug() {
        let error = TemplateError::LoadError {
            name: "test".to_string(),
            source: anyhow::anyhow!("error"),
        };

        let debug = format!("{:?}", error);
        assert!(debug.contains("LoadError"));
    }

    #[test]
    fn compile_error_debug() {
        let error = TemplateError::CompileError {
            name: "test".to_string(),
            source: anyhow::anyhow!("error"),
        };

        let debug = format!("{:?}", error);
        assert!(debug.contains("CompileError"));
    }

    #[test]
    fn render_error_debug() {
        let error = TemplateError::RenderError {
            name: "test".to_string(),
            source: anyhow::anyhow!("error"),
        };

        let debug = format!("{:?}", error);
        assert!(debug.contains("RenderError"));
    }

    #[test]
    fn no_loader_debug() {
        let error = TemplateError::NoLoader("test".to_string());

        let debug = format!("{:?}", error);
        assert!(debug.contains("NoLoader"));
    }

    #[test]
    fn not_initialized_debug() {
        let error = TemplateError::NotInitialized;

        let debug = format!("{:?}", error);
        assert!(debug.contains("NotInitialized"));
    }
}

mod template_error_construction_tests {
    use super::*;

    #[test]
    fn not_found_with_various_names() {
        let names = ["simple", "multi-part-name", "UPPERCASE", "with.dots", "with/slashes"];

        for name in names {
            let error = TemplateError::NotFound(name.to_string());
            assert!(error.to_string().contains(name));
        }
    }

    #[test]
    fn load_error_with_various_sources() {
        let sources = [
            anyhow::anyhow!("io error"),
            anyhow::anyhow!("permission denied"),
            anyhow::anyhow!("network timeout"),
        ];

        for source in sources {
            let error = TemplateError::LoadError {
                name: "test".to_string(),
                source,
            };
            assert!(error.to_string().contains("Failed to load"));
        }
    }

    #[test]
    fn compile_error_preserves_name() {
        let error = TemplateError::CompileError {
            name: "specific-template".to_string(),
            source: anyhow::anyhow!("syntax error at line 5"),
        };

        let display = error.to_string();
        assert!(display.contains("specific-template"));
    }

    #[test]
    fn render_error_preserves_name() {
        let error = TemplateError::RenderError {
            name: "render-template".to_string(),
            source: anyhow::anyhow!("variable 'title' not found"),
        };

        let display = error.to_string();
        assert!(display.contains("render-template"));
    }

    #[test]
    fn empty_template_name_in_not_found() {
        let error = TemplateError::NotFound(String::new());
        let display = error.to_string();
        assert!(display.contains("Template not found"));
    }

    #[test]
    fn empty_template_name_in_no_loader() {
        let error = TemplateError::NoLoader(String::new());
        let display = error.to_string();
        assert!(display.contains("No loader available"));
    }
}

mod error_trait_tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn template_error_is_error() {
        let error = TemplateError::NotFound("test".to_string());
        let _: &dyn Error = &error;
    }

    #[test]
    fn not_found_source_is_none() {
        let error = TemplateError::NotFound("test".to_string());
        assert!(error.source().is_none());
    }

    #[test]
    fn load_error_has_source() {
        let error = TemplateError::LoadError {
            name: "test".to_string(),
            source: anyhow::anyhow!("underlying error"),
        };
        assert!(error.source().is_some());
    }

    #[test]
    fn compile_error_has_source() {
        let error = TemplateError::CompileError {
            name: "test".to_string(),
            source: anyhow::anyhow!("underlying error"),
        };
        assert!(error.source().is_some());
    }

    #[test]
    fn render_error_has_source() {
        let error = TemplateError::RenderError {
            name: "test".to_string(),
            source: anyhow::anyhow!("underlying error"),
        };
        assert!(error.source().is_some());
    }

    #[test]
    fn no_loader_source_is_none() {
        let error = TemplateError::NoLoader("test".to_string());
        assert!(error.source().is_none());
    }

    #[test]
    fn not_initialized_source_is_none() {
        let error = TemplateError::NotInitialized;
        assert!(error.source().is_none());
    }
}

mod edge_case_tests {
    use super::*;

    #[test]
    fn unicode_in_template_name() {
        let error = TemplateError::NotFound("template-日本語".to_string());
        let display = error.to_string();
        assert!(display.contains("日本語"));
    }

    #[test]
    fn special_characters_in_template_name() {
        let error = TemplateError::NotFound("template@special#chars!".to_string());
        let display = error.to_string();
        assert!(display.contains("template@special#chars!"));
    }

    #[test]
    fn long_template_name() {
        let long_name = "x".repeat(1000);
        let error = TemplateError::NotFound(long_name.clone());
        let display = error.to_string();
        assert!(display.contains(&long_name));
    }

    #[test]
    fn newlines_in_template_name() {
        let error = TemplateError::NotFound("line1\nline2".to_string());
        let display = error.to_string();
        assert!(display.contains("line1\nline2"));
    }

    #[test]
    fn nested_error_chain() {
        let inner = anyhow::anyhow!("inner error");
        let middle = anyhow::anyhow!(inner).context("middle context");
        let error = TemplateError::LoadError {
            name: "test".to_string(),
            source: middle,
        };

        let display = error.to_string();
        assert!(display.contains("Failed to load template"));
    }
}
