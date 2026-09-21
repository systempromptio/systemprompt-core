#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::admin::config::ConfigCommands;
use systemprompt_cli::admin::config::secret::{SecretCommands, SetArgs};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_test_fixtures::ensure_test_bootstrap;

fn set(name: &str, value: &str) -> ConfigCommands {
    ConfigCommands::Secret(SecretCommands::Set(SetArgs {
        name: name.to_owned(),
        value: value.to_owned(),
    }))
}

#[tokio::test]
async fn secret_set_preserves_existing_values_and_refused_writes_are_atomic() {
    let boot = ensure_test_bootstrap();
    let mut profile: serde_yaml::Value = serde_yaml::from_str(
        &std::fs::read_to_string(&boot.profile_path).expect("read fixture profile"),
    )
    .expect("parse fixture profile");
    profile["secrets"] = serde_yaml::from_str("source: file\nsecrets_path: secrets.json\n")
        .expect("file secrets section");
    std::fs::write(
        &boot.profile_path,
        serde_yaml::to_string(&profile).expect("serialize fixture profile"),
    )
    .expect("point profile at owned secrets file");
    let secrets_path = boot.profile_path.parent().unwrap().join("secrets.json");
    std::fs::write(&secrets_path, r#"{"existing":"keep-me"}"#).expect("seed owned secrets file");
    let context = CommandContext::new(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    );

    systemprompt_cli::admin::config::execute(set("fixture_provider", "new-secret"), &context)
        .await
        .expect("set provider secret through public config dispatcher");
    let persisted: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&secrets_path).expect("read updated secrets"),
    )
    .expect("updated secrets remain JSON");
    assert_eq!(persisted["existing"], "keep-me");
    assert_eq!(persisted["fixture_provider"], "new-secret");

    let before_reserved = std::fs::read(&secrets_path).expect("snapshot secrets");
    let error = systemprompt_cli::admin::config::execute(
        set("database_url", "postgres://must-not-persist"),
        &context,
    )
    .await
    .expect_err("reserved infrastructure secret must be refused");
    assert!(format!("{error:#}").contains("reserved infrastructure secret"));
    assert_eq!(std::fs::read(&secrets_path).unwrap(), before_reserved);

    std::fs::write(&secrets_path, "{invalid-json").expect("install malformed secrets fixture");
    let malformed_before = std::fs::read(&secrets_path).unwrap();
    let error =
        systemprompt_cli::admin::config::execute(set("fixture_provider", "replacement"), &context)
            .await
            .expect_err("malformed secrets must not be overwritten");
    assert!(format!("{error:#}").contains("Failed to parse secrets"));
    assert_eq!(std::fs::read(&secrets_path).unwrap(), malformed_before);
}
