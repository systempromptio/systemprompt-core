//! The `config migrate` command itself: `--check` fails while anything is
//! pending and writes nothing, a plain run migrates the services tree, the
//! plugin hooks and the retired roles file, and `--profiles` drives the
//! profile rewrite.

use std::path::{Path, PathBuf};

use systemprompt_cli::commands::admin::config::migrate::roles::ROLES_FILE;
use systemprompt_cli::commands::admin::config::migrate::{MigrateArgs, execute};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides};

use crate::support::{from_fixture, snapshot};

fn ctx() -> CommandContext {
    CommandContext::new(
        CliConfig::new().with_interactive(false),
        EnvOverrides::default(),
    )
}

fn args(services: Option<&Path>, profiles: Option<&Path>, check: bool) -> MigrateArgs {
    MigrateArgs {
        check,
        data: false,
        services: services.map(Path::to_path_buf),
        profiles: profiles.map(Path::to_path_buf),
    }
}

async fn run(args: &MigrateArgs) -> Result<(), String> {
    execute(args, &ctx()).await.map_err(|e| format!("{e:#}"))
}

fn write(path: PathBuf, body: &str) -> PathBuf {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(&path, body).expect("write");
    path
}

#[tokio::test]
async fn check_fails_on_a_legacy_tree_and_writes_nothing() {
    let tree = from_fixture("core-default");
    let before = snapshot(&tree.services);
    let error = run(&args(Some(&tree.services), None, true))
        .await
        .expect_err("pending");
    assert!(error.contains("a catalog migration is pending"), "{error}");
    assert_eq!(snapshot(&tree.services), before);
}

#[tokio::test]
async fn a_run_migrates_then_check_and_a_rerun_are_clean() {
    let tree = from_fixture("core-default");
    run(&args(Some(&tree.services), None, false))
        .await
        .expect("migrates");
    assert!(tree.services.join("ai/legacy_map.yaml").is_file());
    run(&args(Some(&tree.services), None, true))
        .await
        .expect("nothing pending");

    let hook = write(
        tree.services.join("plugins/commons/config.yaml"),
        "plugin:\n  id: commons\n  hooks:\n    evaluation: true\n",
    );
    write(tree.services.join(ROLES_FILE), crate::roles::ROLES);
    run(&args(Some(&tree.services), None, false))
        .await
        .expect("a rerun folds hooks and roles");
    assert!(
        std::fs::read_to_string(&hook)
            .expect("hook")
            .contains("judge: true")
    );
    assert!(!tree.services.join(ROLES_FILE).exists());

    let after = snapshot(&tree.services);
    run(&args(Some(&tree.services), None, false))
        .await
        .expect("a third run is a no-op");
    assert_eq!(snapshot(&tree.services), after);
}

#[tokio::test]
async fn check_refuses_a_pending_roles_file_or_hooks_key() {
    let tree = from_fixture("core-default");
    run(&args(Some(&tree.services), None, false))
        .await
        .expect("migrates");

    let roles = write(tree.services.join(ROLES_FILE), crate::roles::ROLES);
    let error = run(&args(Some(&tree.services), None, true))
        .await
        .expect_err("roles pending");
    assert!(error.contains(ROLES_FILE), "{error}");
    std::fs::remove_file(roles).expect("rm roles");

    write(
        tree.services.join("plugins/commons/config.yaml"),
        "plugin:\n  id: commons\n  hooks:\n    evaluation: true\n",
    );
    let error = run(&args(Some(&tree.services), None, true))
        .await
        .expect_err("hooks pending");
    assert!(error.contains("hooks.evaluation"), "{error}");
}

#[tokio::test]
async fn without_services_or_a_profile_the_command_needs_a_profile() {
    let error = run(&args(None, None, true)).await.expect_err("no profile");
    assert!(error.contains("requires a profile"), "{error}");
}

#[tokio::test]
async fn profiles_check_fails_while_pending_then_a_run_clears_it() {
    let tree = crate::profiles::tree();
    let root = crate::profiles::profiles(&tree);

    let error = run(&args(None, Some(&root), true))
        .await
        .expect_err("pending");
    assert!(error.contains("a profile migration is pending"), "{error}");
    assert!(root.join("prod.secrets.profile.yaml").exists());

    run(&args(None, Some(&root), false))
        .await
        .expect("migrates");
    assert!(root.join("prod").join("profile.yaml").is_file());
    run(&args(None, Some(&root), true))
        .await
        .expect("nothing pending");
}
