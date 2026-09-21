#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use systemprompt_cli::cloud::profile::{self, ProfileCommands, ShowFilter};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_test_fixtures::ensure_test_bootstrap;

const HELPER: &str = "commands::cloud_profile_optional_config_recovery::optional_config_helper";

fn show() -> ProfileCommands {
    ProfileCommands::Show {
        name: None,
        filter: ShowFilter::All,
        json: true,
        yaml: false,
    }
}

#[tokio::test]
#[ignore = "re-executed by profile_show_omits_broken_optional_configs_and_recovers_after_repair"]
async fn optional_config_helper() {
    let boot = ensure_test_bootstrap();
    let skills_dir = boot.services_path.join("skills");
    let content_dir = boot.services_path.join("content");
    std::fs::create_dir_all(&skills_dir).unwrap();
    std::fs::create_dir_all(&content_dir).unwrap();
    let skills = skills_dir.join("skills.yaml");
    let content = content_dir.join("config.yaml");
    std::fs::write(&skills, "skills: [broken: yaml").unwrap();
    std::fs::write(&content, "categories: [broken: yaml").unwrap();

    let context = CommandContext::new(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides {
            profile: Some(boot.profile_path.to_string_lossy().into_owned()),
            ..EnvOverrides::default()
        },
    );
    println!("BEGIN_BROKEN_OPTIONAL");
    profile::execute(Some(show()), &context)
        .await
        .expect("show profile while optional configs are malformed");
    println!("END_BROKEN_OPTIONAL");

    std::fs::write(&skills, "enabled: true\nskills: {}\n").unwrap();
    std::fs::write(&content, "categories: {}\ncontent_sources: {}\n").unwrap();
    println!("BEGIN_REPAIRED_OPTIONAL");
    profile::execute(Some(show()), &context)
        .await
        .expect("show profile after optional configs are repaired");
    println!("END_REPAIRED_OPTIONAL");
}

fn bounded_output(mut command: Command) -> Output {
    let stdout = tempfile::NamedTempFile::new().expect("profile stdout capture");
    let stderr = tempfile::NamedTempFile::new().expect("profile stderr capture");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.reopen().unwrap()))
        .stderr(Stdio::from(stderr.reopen().unwrap()));
    let mut child = command.spawn().expect("spawn profile helper");
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(status) = child.try_wait().expect("poll profile helper") {
            return Output {
                status,
                stdout: std::fs::read(stdout.path()).unwrap(),
                stderr: std::fs::read(stderr.path()).unwrap(),
            };
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let status = child.wait().expect("reap profile helper");
            panic!(
                "profile helper timed out ({status})\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&std::fs::read(stdout.path()).unwrap()),
                String::from_utf8_lossy(&std::fs::read(stderr.path()).unwrap())
            );
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn marked_json(stdout: &str, start: &str, end: &str) -> serde_json::Value {
    let json = stdout
        .split_once(start)
        .and_then(|(_, tail)| tail.split_once(end))
        .map(|(value, _)| value.trim())
        .unwrap_or_else(|| panic!("missing {start}/{end} in {stdout}"));
    serde_json::from_str(json).unwrap_or_else(|error| panic!("invalid JSON: {error}: {json}"))
}

fn has_section(artifact: &serde_json::Value, name: &str) -> bool {
    artifact["sections"]
        .as_array()
        .expect("profile card sections")
        .iter()
        .any(|section| section["heading"] == name)
}

#[test]
fn profile_show_omits_broken_optional_configs_and_recovers_after_repair() {
    let mut command = Command::new(std::env::current_exe().expect("unit-test binary"));
    command.args(["--exact", HELPER, "--ignored", "--nocapture"]);
    let output = bounded_output(command);
    assert!(
        output.status.success(),
        "profile helper failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 profile output");
    let broken = marked_json(&stdout, "BEGIN_BROKEN_OPTIONAL", "END_BROKEN_OPTIONAL");
    let repaired = marked_json(&stdout, "BEGIN_REPAIRED_OPTIONAL", "END_REPAIRED_OPTIONAL");
    assert_eq!(broken["title"], "Profile Configuration");
    assert!(has_section(&broken, "environment"));
    assert!(has_section(&broken, "settings"));
    assert!(!has_section(&broken, "skills"));
    assert!(!has_section(&broken, "content"));
    assert!(has_section(&repaired, "skills"));
    assert!(has_section(&repaired, "content"));
}
