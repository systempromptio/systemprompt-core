//! `cloud deploy` profile selection and the interactive `cloud profile` menu,
//! run inside an owned project whose profile the tests rewrite.

use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides};
use systemprompt_cloud::SessionKey;

use crate::session_project_fixture::Project;

#[test]
fn coverage_deploy_selection_excludes_local_profiles_and_resolves_named_profiles() {
    use systemprompt_cli::ScriptedPrompter;
    use systemprompt_cloud_entry::cli::deploy::select::resolve_profile;
    let project = Project::new("unused", true);
    let config = CliConfig::new()
        .with_interactive(true)
        .with_assume_terminal(true);
    let err = resolve_profile(&ScriptedPrompter::default(), None, &config).unwrap_err();
    assert!(
        err.to_string().contains("No deployable profiles"),
        "{err:#}"
    );
    let yaml = std::fs::read_to_string(&project.profile)
        .unwrap()
        .replace("target: local", "target: cloud");
    std::fs::write(&project.profile, yaml).unwrap();
    let selected = resolve_profile(&ScriptedPrompter::new(["0"]), None, &config).unwrap();
    assert_eq!(selected.path, project.profile);
    assert_eq!(
        selected.cloud.unwrap().tenant_id.unwrap().as_str(),
        "coverage-tenant"
    );
    let explicit = resolve_profile(
        &ScriptedPrompter::default(),
        Some(&pname("coverage")),
        &config,
    )
    .unwrap();
    assert_eq!(explicit.path, project.profile);
    assert!(
        resolve_profile(
            &ScriptedPrompter::default(),
            Some(&pname("missing")),
            &config
        )
        .unwrap_err()
        .to_string()
        .contains("not found")
    );
    let noninteractive = CliConfig::new().with_interactive(false);
    assert!(resolve_profile(&ScriptedPrompter::default(), None, &noninteractive).is_err());
}

fn profile_context(project: &Project, answers: &[&str]) -> CommandContext {
    CommandContext::new(
        CliConfig::new()
            .with_interactive(true)
            .with_assume_terminal(true),
        EnvOverrides {
            profile: Some(project.profile.display().to_string()),
            ..Default::default()
        },
    )
    .with_prompter(Box::new(systemprompt_cli::ScriptedPrompter::new(
        answers.iter().copied(),
    )))
}

#[tokio::test]
async fn coverage_profile_menu_edits_an_existing_profile_and_returns_to_the_menu() {
    let project = Project::new("unused", false);
    let before = systemprompt_loader::ProfileLoader::load_from_path(&project.profile).unwrap();
    systemprompt_cloud_entry::cli::profile::execute(None, &profile_context(&project, &["1", "4", "3"]))
        .await
        .unwrap();
    let after = systemprompt_loader::ProfileLoader::load_from_path(&project.profile).unwrap();
    assert_eq!(after.server.host, before.server.host);
    assert_eq!(after.server.port, before.server.port);
}

#[tokio::test]
async fn coverage_profile_menu_declining_deletion_preserves_profile_and_secrets() {
    let project = Project::new("unused", false);
    let secret = project.profile.parent().unwrap().join("secrets.json");
    std::fs::write(&secret, r#"{"custom":"keep"}"#).unwrap();
    systemprompt_cloud_entry::cli::profile::execute(
        None,
        &profile_context(&project, &["2", "0", "no", "3"]),
    )
    .await
    .unwrap();
    assert!(project.profile.exists());
    assert_eq!(
        std::fs::read_to_string(secret).unwrap(),
        r#"{"custom":"keep"}"#
    );
}

#[tokio::test]
async fn coverage_profile_menu_confirmed_deletion_removes_only_selected_profile() {
    let project = Project::new("unused", false);
    let unrelated = project
        .root
        .path()
        .join(".systemprompt/profiles/not-a-profile");
    std::fs::create_dir(&unrelated).unwrap();
    std::fs::write(unrelated.join("keep.txt"), "keep").unwrap();
    systemprompt_cloud_entry::cli::profile::execute(
        None,
        &profile_context(&project, &["2", "0", "yes", "3"]),
    )
    .await
    .unwrap();
    assert!(!project.profile.parent().unwrap().exists());
    assert!(unrelated.join("keep.txt").exists());
}

#[tokio::test]
async fn coverage_profile_edit_persists_server_security_and_runtime_choices() {
    let project = Project::new("unused", false);
    systemprompt_cloud_entry::cli::profile::execute(
        None,
        &profile_context(
            &project,
            &[
                "1",
                "0",
                "127.0.0.2",
                "8123",
                "https://fixture.example",
                "1",
                "600",
                "1200",
                "2",
                "2",
                "3",
                "4",
                "3",
            ],
        ),
    )
    .await
    .unwrap();
    let after = systemprompt_loader::ProfileLoader::load_from_path(&project.profile).unwrap();
    assert_eq!(after.server.host, "127.0.0.2");
    assert_eq!(after.server.port, 8123);
    assert!(after.server.use_https);
    assert_eq!(after.security.issuer, "https://fixture.example");
    assert_eq!(after.security.access_token_expiration, 600);
    assert_eq!(after.security.refresh_token_expiration, 1200);
    assert_eq!(after.runtime.environment.to_string(), "staging");
}
#[test]
fn deploy_without_profile_refuses_a_session_selected_cloud_profile() {
    use systemprompt_cli::ScriptedPrompter;
    use systemprompt_cloud_entry::cli::deploy::select::resolve_profile;
    use systemprompt_cli::paths::ResolvedPaths;
    use systemprompt_cloud::SessionStore;

    let project = Project::new("unused", true);
    let yaml = std::fs::read_to_string(&project.profile)
        .unwrap()
        .replace("target: local", "target: cloud");
    std::fs::write(&project.profile, yaml).unwrap();
    let sessions_dir = ResolvedPaths::discover().sessions_dir();
    let mut store = SessionStore::new();
    store.set_active_with_profile_path(
        &SessionKey::Local,
        &pname("coverage"),
        project.profile.clone(),
    );
    store
        .save(&sessions_dir)
        .expect("persist the session selection");

    let config = CliConfig::new().with_interactive(false);
    let error = resolve_profile(&ScriptedPrompter::default(), None, &config)
        .expect_err("a session-selected cloud profile is never deployed implicitly");

    assert!(
        error.to_string().contains("--profile is required"),
        "{error:#}"
    );
}

fn pname(name: &str) -> systemprompt_identifiers::ProfileName {
    systemprompt_identifiers::ProfileName::try_new(name).expect("valid ProfileName")
}
