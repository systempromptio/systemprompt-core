#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::CliConfig;
use systemprompt_cli::admin::config::planes::{
    ApplyArgs, DiffArgs, ExportArgs, ModeArg, PlanesCommands, execute,
};
use systemprompt_test_fixtures::ensure_test_bootstrap;

fn config() -> CliConfig {
    CliConfig::new().with_interactive(false)
}

fn apply(plane: &str, reason: &str, entities: Vec<String>) -> PlanesCommands {
    PlanesCommands::Apply(ApplyArgs {
        plane: plane.to_owned(),
        reason: reason.to_owned(),
        mode: ModeArg::Overwrite,
        entities,
    })
}

#[tokio::test]
async fn list_renders_the_registered_planes() {
    ensure_test_bootstrap();
    execute(PlanesCommands::List, &config())
        .await
        .expect("list succeeds against the bootstrapped profile");
}

#[tokio::test]
async fn diff_of_an_unregistered_plane_fails() {
    ensure_test_bootstrap();
    let err = execute(
        PlanesCommands::Diff(DiffArgs {
            plane: "no_such_plane_diff".to_owned(),
        }),
        &config(),
    )
    .await
    .expect_err("an unknown plane has no diff");
    assert!(format!("{err:#}").contains("no_such_plane_diff"), "{err:#}");
}

#[tokio::test]
async fn apply_with_a_blank_reason_is_refused_before_any_write() {
    ensure_test_bootstrap();
    let err = execute(apply("any_plane", "   ", Vec::new()), &config())
        .await
        .expect_err("a blank reason is refused");
    assert!(err.to_string().contains("--reason"), "{err}");
}

#[tokio::test]
async fn apply_of_an_unregistered_plane_fails() {
    ensure_test_bootstrap();
    let result = execute(
        apply("no_such_plane_apply", "ticket-1", Vec::new()),
        &config(),
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn a_scoped_apply_of_an_unregistered_plane_fails() {
    ensure_test_bootstrap();
    let result = execute(
        apply(
            "no_such_plane_scoped",
            "ticket-2",
            vec!["entity-a".to_owned()],
        ),
        &config(),
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn export_of_an_unregistered_plane_fails() {
    ensure_test_bootstrap();
    let result = execute(
        PlanesCommands::Export(ExportArgs {
            plane: "no_such_plane_export".to_owned(),
            out: None,
            force: false,
        }),
        &config(),
    )
    .await;
    assert!(result.is_err());
}
