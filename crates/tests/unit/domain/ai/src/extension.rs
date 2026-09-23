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
    fn schemas_returns_expected_tables() {
        let ext = AiExtension;
        let schemas = Extension::schemas(&ext);
        let names: Vec<&str> = schemas.iter().filter_map(|s| s.table.as_deref()).collect();
        assert_eq!(
            names,
            vec![
                "ai_requests",
                "ai_request_client_evidence",
                "ai_request_messages",
                "ai_request_tool_calls",
                "ai_tool_catalogs",
                "ai_request_payloads",
                "ai_safety_findings",
                "ai_quota_buckets",
                "ai_gateway_policies",
                "ai_gateway_thought_signatures",
            ]
        );
    }

    #[test]
    fn schemas_includes_ai_requests() {
        let ext = AiExtension;
        let schemas = Extension::schemas(&ext);
        assert!(
            schemas
                .iter()
                .any(|s| s.table.as_deref() == Some("ai_requests"))
        );
    }

    #[test]
    fn schemas_includes_ai_request_messages() {
        let ext = AiExtension;
        let schemas = Extension::schemas(&ext);
        assert!(
            schemas
                .iter()
                .any(|s| s.table.as_deref() == Some("ai_request_messages"))
        );
    }

    #[test]
    fn schemas_includes_ai_request_tool_calls() {
        let ext = AiExtension;
        let schemas = Extension::schemas(&ext);
        assert!(
            schemas
                .iter()
                .any(|s| s.table.as_deref() == Some("ai_request_tool_calls"))
        );
    }

    #[test]
    fn dependencies_includes_users() {
        let ext = AiExtension;
        let deps = Extension::dependencies(&ext);
        assert!(deps.contains(&"users"));
    }

    #[test]
    fn dependencies_has_two_items() {
        let ext = AiExtension;
        let deps = Extension::dependencies(&ext);
        assert_eq!(deps.len(), 2);
    }


    #[test]
    fn default_creates_extension() {
        let ext = AiExtension::default();
        assert_eq!(Extension::metadata(&ext).id, "ai");
    }
}

#[test]
fn no_reporting_capture_or_privacy_sql_is_registered() {
    assert!(
        AiExtension
            .schemas()
            .iter()
            .all(|schema| !schema.sql.contains("reporting")),
        "the reporting projection is retired"
    );
}

#[test]
fn message_count_is_maintained_by_a_statement_trigger() {
    let schemas = AiExtension.schemas();
    let counter: Vec<_> = schemas
        .iter()
        .filter(|schema| {
            schema.table.is_none()
                && schema
                    .sql
                    .contains("EXECUTE FUNCTION sp_ai_request_message_count")
        })
        .collect();
    assert_eq!(counter.len(), 1, "message_count trigger registered once");
}
