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
fn owner_capture_and_privacy_contracts_are_registered() {
    let schemas = AiExtension.schemas();
    let capture: Vec<_> = schemas
        .iter()
        .filter(|schema| {
            schema.table.is_none()
                && schema
                    .sql
                    .contains("EXECUTE FUNCTION sp_capture_reporting_change")
        })
        .collect();
    assert_eq!(
        capture.len(),
        1,
        "owner capture SQL must be registered exactly once"
    );
    for view in [
        "reporting_source_ai_requests",
        "reporting_source_ai_request_messages",
    ] {
        assert!(
            capture[0].sql.contains(view),
            "missing reporting view: {view}"
        );
    }
    let privacy: Vec<_> = schemas
        .iter()
        .filter(|schema| {
            schema.table.is_none()
                && schema
                    .sql
                    .contains("CREATE OR REPLACE FUNCTION public.lock_ai_reporting_sources")
        })
        .collect();
    assert_eq!(
        privacy.len(),
        1,
        "owner privacy SQL must survive capture registration"
    );
    assert!(privacy[0].sql.contains("reporting_request_is_retained"));
}
