//! Public content mutation commands preserve unrelated content.

use std::sync::Arc;

use clap::Parser;
use systemprompt_cli::core::content::verify::{self, VerifyArgs};
use systemprompt_cli::core::{self, CoreCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_content::ContentRepository;
use systemprompt_content::models::CreateContentParams;
use systemprompt_identifiers::SourceId;
use systemprompt_models::profile::PathsConfig;
use systemprompt_test_fixtures::{
    DisposableDb, ensure_test_bootstrap, fixture_app_context_with, install_test_signing_key,
};

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    command: CoreCommands,
}

fn parse(args: &[&str]) -> CoreCommands {
    Harness::try_parse_from(std::iter::once("core").chain(args.iter().copied()))
        .expect("parse core command")
        .command
}

#[tokio::test]
async fn public_edit_verify_and_delete_mutate_only_the_selected_content() {
    let database = DisposableDb::installed("cli_content_mutation")
        .await
        .expect("private content database");
    let boot = ensure_test_bootstrap();
    install_test_signing_key();
    let pool = database.pool().await.expect("private content pool");
    let repository = ContentRepository::new(&pool).expect("content repository");
    let source = SourceId::new("owned-source".to_owned());
    let target = repository
        .create(
            &CreateContentParams::new(
                "owned-target".to_owned(),
                "Original target".to_owned(),
                "Target description".to_owned(),
                "Original body".to_owned(),
                source.clone(),
            )
            .with_version_hash("target-v1".to_owned()),
        )
        .await
        .expect("seed target content");
    let sibling = repository
        .create(
            &CreateContentParams::new(
                "owned-sibling".to_owned(),
                "Sibling title".to_owned(),
                "Sibling description".to_owned(),
                "Sibling body".to_owned(),
                source,
            )
            .with_version_hash("sibling-v1".to_owned()),
        )
        .await
        .expect("seed sibling content");
    let sibling_before = serde_json::to_value(&sibling).expect("serialize sibling");
    let paths = PathsConfig {
        system: boot.system_path.display().to_string(),
        services: boot.services_path.display().to_string(),
        bin: boot.bin_path.display().to_string(),
        web_path: Some(boot.system_path.join("web").display().to_string()),
        storage: Some(boot.storage_path.display().to_string()),
        geoip_database: None,
    };
    let app = fixture_app_context_with(
        &pool,
        database.url(),
        paths,
        Arc::new(systemprompt_marketplace::AllowAllFilter),
    )
    .expect("full app context");
    let context = CommandContext::with_app_context(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        app,
    );

    core::execute(
        parse(&[
            "content",
            "edit",
            target.id.as_str(),
            "--set",
            "title=Edited target",
            "--body",
            "Edited body",
            "--public",
        ]),
        &context,
    )
    .await
    .expect("public edit command");
    let edited = repository
        .get_by_id(&target.id)
        .await
        .expect("read edited target")
        .expect("edited target remains");
    assert_eq!(edited.title, "Edited target");
    assert_eq!(edited.body, "Edited body");
    assert!(edited.public);

    let dist = tempfile::tempdir().expect("owned web dist");
    let rendered = dist.path().join("owned-source/owned-target/index.html");
    std::fs::create_dir_all(rendered.parent().expect("rendered parent"))
        .expect("create rendered parent");
    std::fs::write(&rendered, "<main>Edited target</main>").expect("write rendered target");
    let verification = verify::execute(
        VerifyArgs {
            identifier: target.id.as_str().to_owned(),
            source: None,
            web_dist: Some(dist.path().to_path_buf()),
            base_url: None,
            url_pattern: None,
        },
        &context,
    )
    .await
    .expect("verification artifact");
    let verification =
        serde_json::to_value(verification.artifact()).expect("serialize verification artifact");
    let sections = verification["sections"]
        .as_array()
        .expect("verification sections");
    let field = |heading: &str| {
        sections
            .iter()
            .find(|section| section["heading"] == heading)
            .unwrap_or_else(|| panic!("missing verification field {heading}: {verification}"))
            ["content"]
            .clone()
    };
    assert_eq!(field("content_id"), target.id.as_str());
    assert_eq!(field("url"), "/owned-source/owned-target");
    assert_eq!(field("prerendered"), true);
    assert_eq!(field("prerender_path"), rendered.display().to_string());
    core::execute(
        parse(&[
            "content",
            "verify",
            target.id.as_str(),
            "--web-dist",
            dist.path().to_str().expect("UTF-8 dist path"),
        ]),
        &context,
    )
    .await
    .expect("public verify command");

    core::execute(
        parse(&["content", "delete", target.id.as_str(), "--yes"]),
        &context,
    )
    .await
    .expect("public delete command");
    assert!(
        repository
            .get_by_id(&target.id)
            .await
            .expect("read deleted target")
            .is_none()
    );
    let sibling_after = repository
        .get_by_id(&sibling.id)
        .await
        .expect("read sibling")
        .expect("sibling remains");
    assert_eq!(
        serde_json::to_value(sibling_after).expect("serialize sibling after"),
        sibling_before
    );

    drop(context);
    pool.write_pool_arc().expect("write pool").close().await;
    drop(pool);
    database.drop_now().await;
}
