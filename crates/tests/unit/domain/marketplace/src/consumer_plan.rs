use crate::consumer_fixture::fixture_with_metadata;
use systemprompt_models::feedback::EvaluatorClient;

#[tokio::test]
async fn every_host_plan_preserves_multiline_quoted_and_backslash_yaml_scalars() {
    let name = "Skill: \"quoted\"\nC:\\skills\\name";
    let description = "First line: \"quoted\"\nSecond line C:\\skills\\tools\t# literal";
    let fixture = fixture_with_metadata(Some((name, description))).await;
    for host in [
        EvaluatorClient::ClaudeCode,
        EvaluatorClient::ClaudeDesktop,
        EvaluatorClient::OpenCode,
        EvaluatorClient::Codex,
        EvaluatorClient::Hermes,
    ] {
        let plan = fixture
            .repo
            .consumer_installation_plan(
                &fixture.credential.credential,
                &fixture.request.resource_id,
                &fixture.request.publication_id,
                host,
            )
            .await
            .unwrap();
        let file = plan
            .runtime_files
            .iter()
            .find(|file| file.path == "SKILL.md")
            .unwrap();
        let text = std::str::from_utf8(&file.bytes).unwrap();
        let yaml = text
            .strip_prefix("---\n")
            .unwrap()
            .split_once("\n---\n")
            .unwrap()
            .0;
        let value: serde_json::Value = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(value["description"], description);
        let expected_name = match host {
            EvaluatorClient::Codex | EvaluatorClient::Hermes => name,
            _ => "skill",
        };
        assert_eq!(value["name"], expected_name);
        assert!(text.ends_with("# Skill\n"));
    }
}
