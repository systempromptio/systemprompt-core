use crate::consumer_fixture::fixture_with_metadata;
use systemprompt_marketplace::managed::ManagedError;
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

    fixture.grant(false).await;
    let denied = fixture
        .repo
        .consumer_installation_plan(
            &fixture.credential.credential,
            &fixture.request.resource_id,
            &fixture.request.publication_id,
            EvaluatorClient::Codex,
        )
        .await
        .expect_err("a revoked consumer must not receive the publication plan");
    assert!(matches!(denied, ManagedError::Unavailable));

    fixture.grant(true).await;
    let restored = fixture
        .repo
        .consumer_installation_plan(
            &fixture.credential.credential,
            &fixture.request.resource_id,
            &fixture.request.publication_id,
            EvaluatorClient::Codex,
        )
        .await
        .expect("restoring the grant recovers the same publication plan");
    assert_eq!(restored.resource_id, fixture.request.resource_id);
    assert_eq!(restored.publication_id, fixture.request.publication_id);
    assert_eq!(restored.generation, fixture.request.generation);
    assert!(
        restored
            .runtime_files
            .iter()
            .any(|file| file.path == "SKILL.md")
    );
}

async fn rendered_skill_md(
    fixture: &crate::consumer_fixture::Fixture,
    host: EvaluatorClient,
) -> String {
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
    String::from_utf8(file.bytes.clone()).unwrap()
}

#[tokio::test]
async fn claude_hosts_receive_the_authored_frontmatter_and_other_hosts_do_not() {
    let config: &[u8] = b"id: skill\nname: Skill\ndescription: d\nfile: SKILL.md\n\
        frontmatter:\n  allowed-tools:\n  - Read\n  title: platform owned\n  user-invocable: false\n";
    let fixture =
        crate::consumer_fixture::fixture_with_extra_files(&[("config.yaml", config)]).await;
    for host in [EvaluatorClient::ClaudeCode, EvaluatorClient::ClaudeDesktop] {
        assert_eq!(
            rendered_skill_md(&fixture, host).await,
            "---\nname: \"skill\"\ndescription: \"d\"\nallowed-tools:\n- Read\nuser-invocable: false\n---\n\n# Skill\n"
        );
    }
    for host in [
        EvaluatorClient::OpenCode,
        EvaluatorClient::Codex,
        EvaluatorClient::Hermes,
    ] {
        let md = rendered_skill_md(&fixture, host).await;
        assert!(!md.contains("allowed-tools"), "{host:?}: {md}");
    }

    let authored: &[u8] =
        b"---\nname: skill\ndescription: d\nargument-hint: \"[file]\"\n---\n# Skill";
    let bare = crate::consumer_fixture::fixture_with_extra_files(&[("SKILL.md", authored)]).await;
    assert_eq!(
        rendered_skill_md(&bare, EvaluatorClient::ClaudeCode).await,
        "---\nname: \"skill\"\ndescription: \"\"\nargument-hint: '[file]'\n---\n\n# Skill\n"
    );
}

#[tokio::test]
async fn malformed_authored_frontmatter_fails_the_plan_instead_of_being_dropped() {
    let authored: &[u8] = b"---\nallowed-tools: [Read\n---\n# Skill";
    let fixture =
        crate::consumer_fixture::fixture_with_extra_files(&[("SKILL.md", authored)]).await;
    let err = fixture
        .repo
        .consumer_installation_plan(
            &fixture.credential.credential,
            &fixture.request.resource_id,
            &fixture.request.publication_id,
            EvaluatorClient::ClaudeCode,
        )
        .await
        .expect_err("a SKILL.md whose frontmatter does not parse must not ship without it");
    assert!(matches!(err, ManagedError::Integrity), "{err:?}");
}
