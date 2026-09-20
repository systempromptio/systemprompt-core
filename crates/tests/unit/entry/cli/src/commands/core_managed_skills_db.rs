#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::BTreeMap;

use clap::Parser;
use systemprompt_cli::core::skills::list::show_resolved_skill;
use systemprompt_cli::core::{self, CoreCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_marketplace::managed::{
    AssetDigest, AssetFile, ComparisonEvidence, NewResource, NewRevision, PublicationAction,
    PublicationRequest, ResourceKind, RevisionFiles, SnapshotProvenance, SourceSpec,
};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context, fixture_db_pool, seed_user_row,
};
use uuid::Uuid;

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    command: CoreCommands,
}

struct ManagedSkillFixture {
    ctx: CommandContext,
    key: String,
    resource: systemprompt_identifiers::ManagedResourceId,
    revision: systemprompt_identifiers::ResourceRevisionId,
}

fn command(args: &[&str]) -> CoreCommands {
    Harness::try_parse_from(std::iter::once("core").chain(args.iter().copied()))
        .unwrap()
        .command
}

fn revision_files(key: &str) -> RevisionFiles {
    let mut files = BTreeMap::new();
    files.insert(
        "config.yaml".to_owned(),
        AssetFile {
            bytes: format!(
                "id: {key}\nname: Published {key}\ndescription: managed description\nenabled: true\n"
            )
            .into_bytes(),
            media_type: "application/yaml".to_owned(),
            executable: false,
        },
    );
    files.insert(
        "index.md".to_owned(),
        AssetFile {
            bytes: b"# published instructions\n\nUse the managed procedure.".to_vec(),
            media_type: "text/markdown".to_owned(),
            executable: false,
        },
    );
    RevisionFiles(files)
}

fn seed_disk_skill(key: &str) {
    let root = ensure_test_bootstrap()
        .services_path
        .join("skills")
        .join(key);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("config.yaml"),
        format!(
            "id: {key}\nname: Filesystem {key}\ndescription: disk description\nenabled: true\n"
        ),
    )
    .unwrap();
    std::fs::write(root.join("index.md"), "# filesystem instructions\n").unwrap();
}

async fn fixture() -> ManagedSkillFixture {
    let bootstrap = ensure_test_bootstrap();
    let pool = fixture_db_pool(&bootstrap.database_url).await.unwrap();
    let app = fixture_app_context(&pool, &bootstrap.database_url).unwrap();
    let owner = app.system_admin().id().clone();
    seed_user_row(
        &pool,
        &owner,
        &format!("{}@managed.invalid", owner.as_str()),
    )
    .await
    .unwrap();
    let repository = app.managed_repository();
    let key = format!("cli_managed_{}", Uuid::new_v4().simple());
    seed_disk_skill(&key);
    let source = repository
        .register_source(
            &owner,
            &format!("cli-source-{}", Uuid::new_v4().simple()),
            &SourceSpec::Managed,
        )
        .await
        .unwrap();
    let snapshot = repository
        .capture_snapshot(
            &owner,
            &source,
            &SnapshotProvenance {
                source_kind: "managed".to_owned(),
                commit: None,
                tree_digest: AssetDigest::of(key.as_bytes()),
                importer_version: "cli-test".to_owned(),
            },
        )
        .await
        .unwrap();
    let resource = repository
        .bind_resource(
            &owner,
            &NewResource {
                source_id: source,
                upstream_key: key.clone(),
                kind: ResourceKind::Skill,
                resource_key: key.clone(),
            },
        )
        .await
        .unwrap();
    let revision = repository
        .create_revision(
            &owner,
            &NewRevision {
                resource_id: resource.clone(),
                snapshot_id: snapshot,
                parent_id: None,
                files: revision_files(&key),
                dependencies: BTreeMap::new(),
                rationale: "CLI managed-skill fixture".to_owned(),
            },
        )
        .await
        .unwrap();
    ManagedSkillFixture {
        ctx: CommandContext::with_app_context(
            CliConfig::new()
                .with_interactive(false)
                .with_output_format(OutputFormat::Json),
            EnvOverrides::default(),
            app,
        ),
        key,
        resource,
        revision,
    }
}

async fn publish(fixture: &ManagedSkillFixture) {
    let app = fixture.ctx.app_context().await.unwrap();
    let owner = app.system_admin().id();
    app.managed_repository()
        .review_and_publish(
            owner,
            owner,
            &PublicationRequest {
                resource_id: fixture.resource.clone(),
                revision_id: Some(fixture.revision.clone()),
                action: PublicationAction::InitialAdoption,
                expected_generation: 0,
                operation_key: format!("publish-{}", fixture.key),
                comparison_evidence: ComparisonEvidence::default(),
                limitations: String::new(),
            },
        )
        .await
        .unwrap();
}

async fn withdraw(fixture: &ManagedSkillFixture) {
    let app = fixture.ctx.app_context().await.unwrap();
    let owner = app.system_admin().id();
    app.managed_repository()
        .review_and_publish(
            owner,
            owner,
            &PublicationRequest {
                resource_id: fixture.resource.clone(),
                revision_id: None,
                action: PublicationAction::Withdraw,
                expected_generation: 1,
                operation_key: format!("withdraw-{}", fixture.key),
                comparison_evidence: ComparisonEvidence::default(),
                limitations: String::new(),
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn published_managed_skill_detail_replaces_same_named_filesystem_content() {
    let fixture = fixture().await;
    publish(&fixture).await;

    core::execute(
        command(&["skills", "show", fixture.key.as_str()]),
        &fixture.ctx,
    )
    .await
    .unwrap();
    let output = show_resolved_skill(&fixture.key, &fixture.ctx)
        .await
        .unwrap();
    let wire = serde_json::to_value(output.artifact()).unwrap().to_string();
    assert!(wire.contains("managed://"), "{wire}");
    assert!(wire.contains(&fixture.key), "{wire}");
    assert!(wire.contains("published instructions"), "{wire}");
    assert!(wire.contains("managed description"), "{wire}");
    assert!(wire.contains("managed generation 1"), "{wire}");
    assert!(!wire.contains("Filesystem"), "{wire}");
    assert!(!wire.contains("filesystem instructions"), "{wire}");
}

#[tokio::test]
async fn never_adopted_managed_skill_suppresses_same_named_disk_skill() {
    let fixture = fixture().await;

    let error = core::execute(
        command(&["skills", "show", fixture.key.as_str()]),
        &fixture.ctx,
    )
    .await
    .unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains("managed but withheld"), "{message}");
    assert!(message.contains("never adopted"), "{message}");
    assert!(!message.contains("disk description"), "{message}");
}

#[tokio::test]
async fn withdrawn_managed_skill_hides_disk_fallback_and_named_lookup_errors() {
    let fixture = fixture().await;
    publish(&fixture).await;
    withdraw(&fixture).await;

    let error = core::execute(
        command(&["skills", "show", fixture.key.as_str()]),
        &fixture.ctx,
    )
    .await
    .unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains("managed but withheld"), "{message}");
    assert!(message.contains("withdrawn"), "{message}");
    assert!(!message.contains("filesystem instructions"), "{message}");
}

const LIST_HELPER: &str = "commands::core_managed_skills_db::managed_list_output_helper";

#[tokio::test]
#[ignore = "re-executed by managed_list_output_reflects_publication_lifecycle"]
async fn managed_list_output_helper() {
    let fixture = fixture().await;
    publish(&fixture).await;

    println!("FIXTURE_KEY={}", fixture.key);
    println!("BEGIN_PUBLISHED_LIST");
    core::execute(command(&["skills", "list"]), &fixture.ctx)
        .await
        .expect("published managed skill list");
    println!("END_PUBLISHED_LIST");

    withdraw(&fixture).await;
    println!("BEGIN_WITHDRAWN_LIST");
    core::execute(command(&["skills", "list"]), &fixture.ctx)
        .await
        .expect("withdrawn managed skill list");
    println!("END_WITHDRAWN_LIST");
}

#[test]
fn managed_list_output_reflects_publication_lifecycle() {
    let output =
        std::process::Command::new(std::env::current_exe().expect("unit-test binary path"))
            .args(["--exact", LIST_HELPER, "--ignored", "--nocapture"])
            .output()
            .expect("re-execute managed list helper");

    assert!(
        output.status.success(),
        "managed list helper failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("JSON output is UTF-8");
    let published = stdout
        .split_once("BEGIN_PUBLISHED_LIST")
        .and_then(|(_, tail)| tail.split_once("END_PUBLISHED_LIST"))
        .map(|(section, _)| section)
        .expect("published output markers");
    let withdrawn = stdout
        .split_once("BEGIN_WITHDRAWN_LIST")
        .and_then(|(_, tail)| tail.split_once("END_WITHDRAWN_LIST"))
        .map(|(section, _)| section)
        .expect("withdrawn output markers");

    let key = stdout
        .lines()
        .find_map(|line| line.strip_prefix("FIXTURE_KEY="))
        .expect("helper reports its unique fixture key");
    let published: serde_json::Value =
        serde_json::from_str(published.trim()).expect("published list is JSON");
    let withdrawn: serde_json::Value =
        serde_json::from_str(withdrawn.trim()).expect("withdrawn list is JSON");
    let published_rows = published["items"].as_array().expect("published table rows");
    let row = published_rows
        .iter()
        .find(|row| row["skill_id"] == key)
        .unwrap_or_else(|| panic!("missing exact fixture row {key}: {published}"));
    assert_eq!(row["name"], format!("Published {key}"), "{published}");
    assert!(
        row["file_path"]
            .as_str()
            .is_some_and(|path| path.starts_with("managed://") && path.contains('@')),
        "{published}"
    );
    assert_ne!(row["name"], format!("Filesystem {key}"), "{published}");
    assert!(
        withdrawn["items"]
            .as_array()
            .expect("withdrawn table rows")
            .iter()
            .all(|row| row["skill_id"] != key),
        "withdrawal must suppress the same-key disk fallback: {withdrawn}"
    );
}
