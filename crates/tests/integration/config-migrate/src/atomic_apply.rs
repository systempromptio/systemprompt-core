//! `config migrate` writes its plan all-or-nothing: every file is staged next
//! to its target before the first one replaces the live tree.

use std::os::unix::fs::PermissionsExt;

use systemprompt_cli::commands::admin::config::migrate::{plan, write};

use crate::support::{from_files, snapshot};

const PROVIDERS: &str = r"
providers:
  - name: alpha
    wire: anthropic
    surface: anthropic
    endpoint: https://api.anthropic.com/v1
    api_key_secret: alpha
    governance: { european: true }
    models:
      - id: alpha-1
        pricing: { input_per_million: 1.0, output_per_million: 2.0, cache_read_per_million: 0.1 }
";

const ROOT: &str = "includes:\n  - ../ai/providers.yaml\n";

fn set_mode(path: &std::path::Path, mode: u32) {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("chmod");
}

#[test]
fn a_target_that_cannot_be_written_leaves_the_whole_tree_untouched() {
    let tree = from_files(&[("providers.yaml", PROVIDERS)]);
    let locked = tempfile::tempdir().expect("locked dir");
    std::fs::write(locked.path().join("config.yaml"), ROOT).expect("root config");
    let config_dir = tree.services.join("config");
    std::fs::remove_dir_all(&config_dir).expect("drop the plain config dir");
    std::os::unix::fs::symlink(locked.path(), &config_dir).expect("link the locked config dir");
    let pending = plan(&tree.services).expect("plans").expect("pending");
    assert!(
        pending.root_config.is_some(),
        "the fixture must rewrite the root config so the last write can fail"
    );

    let before = snapshot(&tree.services);
    set_mode(&locked.path().join("config.yaml"), 0o444);
    set_mode(locked.path(), 0o555);
    let outcome = write::apply(&tree.services, &pending);
    set_mode(locked.path(), 0o755);
    set_mode(&locked.path().join("config.yaml"), 0o644);

    let err = outcome.expect_err("a read-only root config stops the migration");
    let message = format!("{err:#}");
    assert!(message.contains("nothing was changed"), "{message}");
    assert_eq!(
        snapshot(&tree.services),
        before,
        "a failed migration rewrote part of the services tree"
    );
}

#[test]
fn a_successful_apply_leaves_no_staging_files_behind() {
    let tree = from_files(&[("providers.yaml", PROVIDERS)]);
    std::fs::write(tree.root(), ROOT).expect("root config");
    let pending = plan(&tree.services).expect("plans").expect("pending");

    let written = write::apply(&tree.services, &pending).expect("migration applies");

    assert!(written.iter().any(|p| p.ends_with("config/config.yaml")));
    let leftovers: Vec<_> = snapshot(&tree.services)
        .into_iter()
        .map(|(p, _)| p)
        .filter(|p| p.to_string_lossy().contains(".migrate-"))
        .collect();
    assert!(leftovers.is_empty(), "staging files left: {leftovers:?}");
    let root = std::fs::read_to_string(tree.root()).expect("root");
    assert!(!root.contains("../ai/providers.yaml"), "{root}");
}
