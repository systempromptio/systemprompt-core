use systemprompt_ai::AiExtension;
use systemprompt_extension::Extension;

mod ai_extension_tests {
    use super::*;

    #[test]
    fn metadata_has_correct_id() {
        let ext = AiExtension;
        let metadata = Extension::metadata(&ext);
        assert_eq!(metadata.id, "ai");
    }

    #[test]
    fn metadata_has_correct_name() {
        let ext = AiExtension;
        let metadata = Extension::metadata(&ext);
        assert_eq!(metadata.name, "AI");
    }

    #[test]
    fn metadata_has_version() {
        let ext = AiExtension;
        let metadata = Extension::metadata(&ext);
        assert!(!metadata.version.is_empty());
    }

    #[test]
    fn migration_weight_is_20() {
        let ext = AiExtension;
        assert_eq!(Extension::migration_weight(&ext), 20);
    }

    #[test]
    fn schemas_returns_three_tables() {
        let ext = AiExtension;
        let schemas = Extension::schemas(&ext);
        assert_eq!(schemas.len(), 3);
    }

    #[test]
    fn schemas_includes_ai_requests() {
        let ext = AiExtension;
        let schemas = Extension::schemas(&ext);
        assert!(schemas.iter().any(|s| s.table == "ai_requests"));
    }

    #[test]
    fn schemas_includes_ai_request_messages() {
        let ext = AiExtension;
        let schemas = Extension::schemas(&ext);
        assert!(schemas.iter().any(|s| s.table == "ai_request_messages"));
    }

    #[test]
    fn schemas_includes_ai_request_tool_calls() {
        let ext = AiExtension;
        let schemas = Extension::schemas(&ext);
        assert!(schemas.iter().any(|s| s.table == "ai_request_tool_calls"));
    }

    #[test]
    fn dependencies_includes_users() {
        let ext = AiExtension;
        let deps = Extension::dependencies(&ext);
        assert!(deps.contains(&"users"));
    }

    #[test]
    fn dependencies_has_one_item() {
        let ext = AiExtension;
        let deps = Extension::dependencies(&ext);
        assert_eq!(deps.len(), 1);
    }

    #[test]
    fn is_debug() {
        let ext = AiExtension;
        let debug_str = format!("{:?}", ext);
        assert!(debug_str.contains("AiExtension"));
    }

    #[test]
    fn is_clone() {
        let ext = AiExtension;
        let cloned = ext.clone();
        assert_eq!(Extension::metadata(&cloned).id, "ai");
    }

    #[test]
    fn is_copy() {
        let ext = AiExtension;
        let copied = ext;
        assert_eq!(Extension::metadata(&copied).id, "ai");
    }

    #[test]
    fn default_creates_extension() {
        let ext = AiExtension::default();
        assert_eq!(Extension::metadata(&ext).id, "ai");
    }
}
