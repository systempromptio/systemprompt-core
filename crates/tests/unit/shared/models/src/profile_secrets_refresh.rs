use systemprompt_manifest::profile::{ProfileError, SecretsConfig};

fn parse(yaml: &str) -> SecretsConfig {
    serde_yaml::from_str(yaml).expect("secrets block parses")
}

#[test]
fn file_source_accepts_a_refresh_interval_at_the_floor() {
    let cfg = parse("source: file\nsecrets_path: /run/secrets.json\nrefresh_secs: 60\n");
    cfg.validate().expect("60 s refresh on a file source");
    assert_eq!(cfg.secrets_path().expect("path"), "/run/secrets.json");
}

#[test]
fn file_source_refresh_below_the_floor_is_rejected() {
    let cfg = parse("source: file\nsecrets_path: /run/secrets.json\nrefresh_secs: 59\n");
    let err = cfg.validate().expect_err("59 s is too frequent");
    assert!(
        matches!(
            err,
            ProfileError::RefreshIntervalOutOfRange { field: "secrets.refresh_secs", secs: 59 }
        ),
        "{err:?}"
    );
}

#[test]
fn refresh_secs_on_a_non_file_source_is_rejected_with_the_source_name() {
    for (source, name) in [("env", "env"), ("vault", "vault"), ("external", "external")] {
        let cfg = parse(&format!("source: {source}\nrefresh_secs: 300\n"));
        let err = cfg.validate().expect_err("refresh only re-reads files");
        assert!(
            matches!(err, ProfileError::RefreshSecsUnexpected { secrets_source } if secrets_source == name),
            "{source}: {err:?}"
        );
    }
}

#[test]
fn external_source_requires_its_block() {
    let err = parse("source: external\n").validate().expect_err("no block");
    assert!(matches!(err, ProfileError::ExternalBlockRequired), "{err:?}");
}

#[test]
fn external_source_with_a_block_validates_and_keeps_provider_keys() {
    let cfg = parse("source: external\nexternal:\n  kind: aws-sm\n  region: eu-west-1\n  refresh_secs: 120\n");
    cfg.validate().expect("valid external source");
    let external = cfg.external.as_ref().expect("block");
    assert_eq!(external.kind, "aws-sm");
    assert_eq!(external.config.get("region").and_then(|v| v.as_str()), Some("eu-west-1"));
    assert_eq!(cfg.source_name(), "external");
}

#[test]
fn external_refresh_below_the_floor_is_rejected() {
    let cfg = parse("source: external\nexternal:\n  kind: aws-sm\n  refresh_secs: 5\n");
    let err = cfg.validate().expect_err("5 s is too frequent");
    assert!(
        matches!(
            err,
            ProfileError::RefreshIntervalOutOfRange {
                field: "secrets.external.refresh_secs",
                secs: 5
            }
        ),
        "{err:?}"
    );
}

#[test]
fn an_external_block_on_file_env_or_vault_is_rejected() {
    let external = "external:\n  kind: aws-sm\n";
    let file = parse(&format!("source: file\nsecrets_path: /s.json\n{external}"));
    let env = parse(&format!("source: env\n{external}"));
    for (cfg, name) in [(file, "file"), (env, "env")] {
        let err = cfg.validate().expect_err("external block is foreign");
        assert!(
            matches!(err, ProfileError::ExternalBlockUnexpected { secrets_source } if secrets_source == name),
            "{err:?}"
        );
    }
}

#[test]
fn a_vault_block_on_an_external_source_is_rejected() {
    let cfg = parse(
        "source: external\nexternal:\n  kind: aws-sm\nvault:\n  address: https://vault.example.com\n  path: sp/prod\n  auth:\n    method: token\n",
    );
    let err = cfg.validate().expect_err("vault block is foreign");
    assert!(
        matches!(err, ProfileError::VaultBlockUnexpected { secrets_source: "external" }),
        "{err:?}"
    );
}

#[test]
fn secrets_path_accessor_reports_a_missing_path_for_env() {
    let cfg = parse("source: env\n");
    cfg.validate().expect("env needs no path");
    let err = cfg.secrets_path().expect_err("no path configured");
    assert!(
        matches!(err, ProfileError::SecretsPathRequired { secrets_source: "env" }),
        "{err:?}"
    );
}
