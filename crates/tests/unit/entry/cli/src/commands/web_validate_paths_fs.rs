#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::PathBuf;

use clap::Parser;
use systemprompt_cli::web::{self, WebCommands};
use systemprompt_cli::{CliConfig, EnvOverrides, OutputFormat};

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: WebCommands,
}

fn run(args: &[&str]) -> anyhow::Result<()> {
    let cmd = Harness::try_parse_from(std::iter::once("web").chain(args.iter().copied()))
        .unwrap()
        .cmd;
    let ctx = crate::boot_ctx::new_with_boot(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    );
    web::execute(cmd, &ctx)
}

fn services_root() -> PathBuf {
    systemprompt_test_fixtures::ensure_test_bootstrap()
        .services_path
        .clone()
}

fn remove(path: &std::path::Path) {
    let outcome = if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    match outcome {
        Ok(()) => {},
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
        Err(e) => panic!("remove {}: {e}", path.display()),
    }
}

fn invalid(result: anyhow::Result<()>) {
    let err = result.expect_err("validation must fail");
    let rendered = format!("{err:#}");
    assert!(
        rendered.contains("web configuration is invalid"),
        "{rendered}"
    );
}

#[test]
fn a_missing_web_config_fails_config_validation() {
    let root = services_root();
    remove(&root.join("web/config.yaml"));
    invalid(run(&["validate", "--only", "config"]));
}

#[test]
fn a_missing_content_config_fails_config_validation() {
    let root = services_root();
    std::fs::create_dir_all(root.join("web")).unwrap();
    std::fs::write(root.join("web/config.yaml"), "branding: {}\n").unwrap();
    remove(&root.join("content/config.yaml"));
    invalid(run(&["validate", "--only", "config"]));
}

#[test]
fn an_unreadable_content_config_fails_config_validation() {
    let root = services_root();
    std::fs::create_dir_all(root.join("web")).unwrap();
    std::fs::write(root.join("web/config.yaml"), "branding: {}\n").unwrap();
    let content = root.join("content/config.yaml");
    remove(&content);
    std::fs::create_dir_all(&content).unwrap();
    invalid(run(&["validate", "--only", "config"]));
}

#[test]
fn referenced_logos_and_favicon_must_exist_in_the_assets_dir() {
    let root = services_root();
    let assets = root.join("web/assets");
    remove(&assets);
    std::fs::create_dir_all(&assets).unwrap();
    std::fs::write(
        root.join("web/config.yaml"),
        "branding: {}\n# header uses logo.svg and logo.png, tab uses the favicon\n",
    )
    .unwrap();

    invalid(run(&["validate", "--only", "assets"]));

    std::fs::create_dir_all(assets.join("logos")).unwrap();
    std::fs::write(assets.join("logos/logo.svg"), "<svg/>").unwrap();
    invalid(run(&["validate", "--only", "assets"]));

    std::fs::write(assets.join("logos/logo.png"), b"\x89PNG").unwrap();
    run(&["validate", "--only", "assets"]).expect("every referenced asset exists");
}

#[test]
fn a_favicon_ico_satisfies_the_favicon_reference() {
    let root = services_root();
    let assets = root.join("web/assets");
    remove(&assets);
    std::fs::create_dir_all(&assets).unwrap();
    std::fs::write(root.join("web/config.yaml"), "branding: {}\n# favicon\n").unwrap();
    invalid(run(&["validate", "--only", "assets"]));
    std::fs::write(assets.join("favicon.ico"), b"ico").unwrap();
    run(&["validate", "--only", "assets"]).expect("favicon.ico present");
}
