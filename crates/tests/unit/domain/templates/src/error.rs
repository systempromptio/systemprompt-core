use systemprompt_templates::TemplateError;

mod template_error_display_tests {
    use super::*;

    #[test]
    fn not_found_displays_template_name() {
        let error = TemplateError::NotFound("my-template".to_string());

        let display = error.to_string();
        assert!(display.contains("template not found"));
        assert!(display.contains("my-template"));
    }

    #[test]
    fn load_error_displays_name_and_message() {
        let error = TemplateError::LoadError {
            name: "broken-template".to_string(),
            message: "file not accessible".to_string(),
        };

        let display = error.to_string();
        assert!(display.contains("failed to load template"));
        assert!(display.contains("broken-template"));
        assert!(display.contains("file not accessible"));
    }

    #[test]
    fn compile_error_displays_name_and_message() {
        let error = TemplateError::CompileError {
            name: "invalid-syntax".to_string(),
            message: "unexpected token".to_string(),
        };

        let display = error.to_string();
        assert!(display.contains("failed to compile template"));
        assert!(display.contains("invalid-syntax"));
        assert!(display.contains("unexpected token"));
    }

    #[test]
    fn render_error_displays_name_and_message() {
        let error = TemplateError::RenderError {
            name: "render-fail".to_string(),
            message: "missing variable".to_string(),
        };

        let display = error.to_string();
        assert!(display.contains("failed to render template"));
        assert!(display.contains("render-fail"));
        assert!(display.contains("missing variable"));
    }

    #[test]
    fn no_loader_displays_template_name() {
        let error = TemplateError::NoLoader("orphan-template".to_string());

        let display = error.to_string();
        assert!(display.contains("no loader available"));
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
            message: "error".to_string(),
        };

        let debug = format!("{:?}", error);
        assert!(debug.contains("LoadError"));
    }

    #[test]
    fn compile_error_debug() {
        let error = TemplateError::CompileError {
            name: "test".to_string(),
            message: "error".to_string(),
        };

        let debug = format!("{:?}", error);
        assert!(debug.contains("CompileError"));
    }

    #[test]
    fn render_error_debug() {
        let error = TemplateError::RenderError {
            name: "test".to_string(),
            message: "error".to_string(),
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
        let names = [
            "simple",
            "multi-part-name",
            "UPPERCASE",
            "with.dots",
            "with/slashes",
        ];

        for name in names {
            let error = TemplateError::NotFound(name.to_string());
            assert!(error.to_string().contains(name));
        }
    }

    #[test]
    fn load_error_with_various_messages() {
        let messages = ["io error", "permission denied", "network timeout"];

        for message in messages {
            let error = TemplateError::LoadError {
                name: "test".to_string(),
                message: message.to_string(),
            };
            assert!(error.to_string().contains("failed to load"));
            assert!(error.to_string().contains(message));
        }
    }

    #[test]
    fn compile_error_preserves_name() {
        let error = TemplateError::CompileError {
            name: "specific-template".to_string(),
            message: "syntax error at line 5".to_string(),
        };

        let display = error.to_string();
        assert!(display.contains("specific-template"));
    }

    #[test]
    fn render_error_preserves_name() {
        let error = TemplateError::RenderError {
            name: "render-template".to_string(),
            message: "variable 'title' not found".to_string(),
        };

        let display = error.to_string();
        assert!(display.contains("render-template"));
    }

    #[test]
    fn empty_template_name_in_not_found() {
        let error = TemplateError::NotFound(String::new());
        let display = error.to_string();
        assert!(display.contains("template not found"));
    }

    #[test]
    fn empty_template_name_in_no_loader() {
        let error = TemplateError::NoLoader(String::new());
        let display = error.to_string();
        assert!(display.contains("no loader available"));
    }
}

mod error_trait_tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn not_found_source_is_none() {
        let error = TemplateError::NotFound("test".to_string());
        assert!(error.source().is_none());
    }

    #[test]
    fn load_error_source_is_none() {
        let error = TemplateError::LoadError {
            name: "test".to_string(),
            message: "underlying error".to_string(),
        };
        assert!(error.source().is_none());
    }

    #[test]
    fn compile_error_source_is_none() {
        let error = TemplateError::CompileError {
            name: "test".to_string(),
            message: "underlying error".to_string(),
        };
        assert!(error.source().is_none());
    }

    #[test]
    fn render_error_source_is_none() {
        let error = TemplateError::RenderError {
            name: "test".to_string(),
            message: "underlying error".to_string(),
        };
        assert!(error.source().is_none());
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
    fn nested_error_message() {
        let error = TemplateError::LoadError {
            name: "test".to_string(),
            message: "middle context: inner error".to_string(),
        };

        let display = error.to_string();
        assert!(display.contains("failed to load template"));
        assert!(display.contains("middle context"));
    }
}
