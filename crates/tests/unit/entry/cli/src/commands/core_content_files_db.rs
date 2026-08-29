//! DB-backed tests for the pool-seamed `core content files` command tree
//! (list/link/unlink/featured), driving `execute_with_pool` against a fixture
//! pool with real content and file rows.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use serde_json::Value;
use systemprompt_cli::CliConfig;
use systemprompt_cli::core::content::files::{featured, link, list, unlink};
use systemprompt_cli::interactive::ScriptedPrompter;
use systemprompt_cli::shared::CommandOutput;
use systemprompt_content::ContentRepository;
use systemprompt_content::models::CreateContentParams;
use systemprompt_database::DbPool;
use systemprompt_files::{FileRepository, FileRole};
use systemprompt_identifiers::{ContentId, FileId, SourceId};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
use uuid::Uuid;

async fn pool() -> DbPool {
    fixture_db_pool(&fixture_database_url().unwrap())
        .await
        .unwrap()
}

fn cfg() -> CliConfig {
    CliConfig::new().with_interactive(false)
}

fn artifact_json(out: &CommandOutput) -> Value {
    serde_json::to_value(out.artifact()).unwrap()
}

fn contains(out: &CommandOutput, needle: &str) -> bool {
    serde_json::to_string(&artifact_json(out))
        .unwrap()
        .contains(needle)
}

async fn seed_content(pool: &DbPool) -> ContentId {
    let repo = ContentRepository::new(pool).unwrap();
    let slug = format!("cf-{}", Uuid::new_v4().simple());
    let params = CreateContentParams::new(
        slug.clone(),
        format!("Title {slug}"),
        "desc".to_owned(),
        "body".to_owned(),
        SourceId::new(format!("src-{}", Uuid::new_v4().simple())),
    )
    .with_version_hash("h1".to_owned());
    repo.create(&params).await.unwrap().id
}

async fn seed_file(pool: &DbPool) -> String {
    let id = Uuid::new_v4();
    let path = format!("/uploads/cf/{id}.png");
    let url = format!("https://files.invalid/{id}");
    sqlx::query(
        "INSERT INTO files (id, path, public_url, mime_type, size_bytes, ai_content) \
         VALUES ($1, $2, $3, 'image/png', 64, false)",
    )
    .bind(id)
    .bind(&path)
    .bind(&url)
    .execute(pool.pool_arc().unwrap().as_ref())
    .await
    .unwrap();
    id.to_string()
}

fn link_args(file: &str, content: &ContentId, role: link::FileRoleArg) -> link::LinkArgs {
    link::LinkArgs {
        file: file.to_owned(),
        content: content.as_str().to_owned(),
        role,
        order: 0,
    }
}

async fn linked_roles(pool: &DbPool, file: &str) -> Vec<FileRole> {
    let repo = FileRepository::new(pool).unwrap();
    repo.list_content_by_file(&FileId::new(file.to_owned()))
        .await
        .unwrap()
        .into_iter()
        .map(|cf| cf.role)
        .collect()
}

#[tokio::test]
async fn link_attaches_file_to_content() {
    let pool = pool().await;
    let content = seed_content(&pool).await;
    let file = seed_file(&pool).await;

    let out = link::execute_with_pool(
        link_args(&file, &content, link::FileRoleArg::Attachment),
        &pool,
        &cfg(),
    )
    .await
    .unwrap();

    assert!(contains(&out, &file));
    assert_eq!(linked_roles(&pool, &file).await, vec![FileRole::Attachment]);
}

#[tokio::test]
async fn list_by_content_and_by_file_reflect_link() {
    let pool = pool().await;
    let content = seed_content(&pool).await;
    let file = seed_file(&pool).await;
    link::execute_with_pool(
        link_args(&file, &content, link::FileRoleArg::Inline),
        &pool,
        &cfg(),
    )
    .await
    .unwrap();

    let by_content = list::execute_with_pool(
        list::ListArgs {
            content: Some(content.as_str().to_owned()),
            file: None,
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap();
    assert!(contains(&by_content, &file));

    let by_file = list::execute_with_pool(
        list::ListArgs {
            content: None,
            file: Some(file.clone()),
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap();
    assert!(contains(&by_file, content.as_str()));
}

#[tokio::test]
async fn list_requires_exactly_one_filter() {
    let pool = pool().await;

    let none = list::execute_with_pool(
        list::ListArgs {
            content: None,
            file: None,
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap_err();
    assert!(none.to_string().contains("Either --content or --file"));

    let both = list::execute_with_pool(
        list::ListArgs {
            content: Some("c".to_owned()),
            file: Some("f".to_owned()),
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap_err();
    assert!(both.to_string().contains("Cannot specify both"));
}

#[tokio::test]
async fn list_by_file_rejects_bad_uuid() {
    let pool = pool().await;
    let err = list::execute_with_pool(
        list::ListArgs {
            content: None,
            file: Some("nope".to_owned()),
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("Invalid file ID format"));
}

fn unlink_args(file: &str, content: &ContentId, yes: bool, dry_run: bool) -> unlink::UnlinkArgs {
    unlink::UnlinkArgs {
        file: file.to_owned(),
        content: content.as_str().to_owned(),
        yes,
        dry_run,
    }
}

#[tokio::test]
async fn unlink_with_yes_removes_link() {
    let pool = pool().await;
    let content = seed_content(&pool).await;
    let file = seed_file(&pool).await;
    link::execute_with_pool(
        link_args(&file, &content, link::FileRoleArg::Attachment),
        &pool,
        &cfg(),
    )
    .await
    .unwrap();

    unlink::execute_with_pool(
        unlink_args(&file, &content, true, false),
        &ScriptedPrompter::new(Vec::<String>::new()),
        &pool,
        &cfg(),
    )
    .await
    .unwrap();

    assert!(linked_roles(&pool, &file).await.is_empty());
}

#[tokio::test]
async fn unlink_dry_run_preserves_link() {
    let pool = pool().await;
    let content = seed_content(&pool).await;
    let file = seed_file(&pool).await;
    link::execute_with_pool(
        link_args(&file, &content, link::FileRoleArg::Attachment),
        &pool,
        &cfg(),
    )
    .await
    .unwrap();

    unlink::execute_with_pool(
        unlink_args(&file, &content, true, true),
        &ScriptedPrompter::new(Vec::<String>::new()),
        &pool,
        &cfg(),
    )
    .await
    .unwrap();

    assert_eq!(linked_roles(&pool, &file).await, vec![FileRole::Attachment]);
}

#[tokio::test]
async fn unlink_non_interactive_without_yes_errors() {
    let pool = pool().await;
    let content = seed_content(&pool).await;
    let file = seed_file(&pool).await;
    link::execute_with_pool(
        link_args(&file, &content, link::FileRoleArg::Attachment),
        &pool,
        &cfg(),
    )
    .await
    .unwrap();

    let err = unlink::execute_with_pool(
        unlink_args(&file, &content, false, false),
        &ScriptedPrompter::new(Vec::<String>::new()),
        &pool,
        &cfg(),
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("--yes is required"));
    assert_eq!(linked_roles(&pool, &file).await, vec![FileRole::Attachment]);
}

#[tokio::test]
async fn unlink_interactive_confirm_no_preserves_link() {
    let pool = pool().await;
    let content = seed_content(&pool).await;
    let file = seed_file(&pool).await;
    link::execute_with_pool(
        link_args(&file, &content, link::FileRoleArg::Attachment),
        &pool,
        &cfg(),
    )
    .await
    .unwrap();

    let err = unlink::execute_with_pool(
        unlink_args(&file, &content, false, false),
        &ScriptedPrompter::new(vec!["n"]),
        &pool,
        &CliConfig::new()
            .with_interactive(true)
            .with_assume_terminal(true),
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("cancelled"));
    assert_eq!(linked_roles(&pool, &file).await, vec![FileRole::Attachment]);
}

#[tokio::test]
async fn featured_set_then_get_returns_image() {
    let pool = pool().await;
    let content = seed_content(&pool).await;
    let file = seed_file(&pool).await;
    link::execute_with_pool(
        link_args(&file, &content, link::FileRoleArg::Attachment),
        &pool,
        &cfg(),
    )
    .await
    .unwrap();

    featured::execute_with_pool(
        featured::FeaturedArgs {
            content: content.as_str().to_owned(),
            set: Some(file.clone()),
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap();

    assert_eq!(linked_roles(&pool, &file).await, vec![FileRole::Featured]);

    let out = featured::execute_with_pool(
        featured::FeaturedArgs {
            content: content.as_str().to_owned(),
            set: None,
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap();
    assert!(contains(&out, &file));
}

#[tokio::test]
async fn featured_get_none_reports_absence() {
    let pool = pool().await;
    let content = seed_content(&pool).await;

    let out = featured::execute_with_pool(
        featured::FeaturedArgs {
            content: content.as_str().to_owned(),
            set: None,
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap();

    assert!(contains(&out, "No featured image set"));
}

#[tokio::test]
async fn featured_set_unlinked_file_errors() {
    let pool = pool().await;
    let content = seed_content(&pool).await;
    let file = seed_file(&pool).await;

    let err = featured::execute_with_pool(
        featured::FeaturedArgs {
            content: content.as_str().to_owned(),
            set: Some(file),
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("not linked"));
}

// The dispatcher in `files/mod.rs` was at 0%: every test above calls a
// subcommand directly through its pool seam, so nothing exercised the routing
// that decides which subcommand a parsed variant reaches. These drive
// `ContentFilesCommands` and assert the effect in the database, because a
// variant wired to the wrong arm would still return `Ok(())`.
mod dispatch {
    use super::{cfg, link, linked_roles, pool, seed_content, seed_file};
    use systemprompt_cli::core::content::files::{
        ContentFilesCommands, execute, featured, list, unlink,
    };
    use systemprompt_cli::interactive::ScriptedPrompter;
    use systemprompt_cli::{CommandContext, EnvOverrides, OutputFormat};
    use systemprompt_database::DbPool;
    use systemprompt_files::FileRole;
    use systemprompt_test_fixtures::{fixture_app_context, fixture_database_url};

    fn ctx(pool: &DbPool) -> CommandContext {
        let url = fixture_database_url().expect("DATABASE_URL");
        CommandContext::with_app_context(
            cfg().with_output_format(OutputFormat::Json),
            EnvOverrides::default(),
            fixture_app_context(pool, &url).expect("app context"),
        )
    }

    async fn dispatch(pool: &DbPool, cmd: ContentFilesCommands) -> anyhow::Result<()> {
        execute(cmd, &ScriptedPrompter::default(), &ctx(pool)).await
    }

    #[tokio::test]
    async fn the_link_variant_reaches_link_and_creates_the_association() {
        let pool = pool().await;
        let content = seed_content(&pool).await;
        let file = seed_file(&pool).await;

        dispatch(
            &pool,
            ContentFilesCommands::Link(link::LinkArgs {
                file: file.clone(),
                content: content.as_str().to_owned(),
                role: link::FileRoleArg::Attachment,
                order: 0,
            }),
        )
        .await
        .expect("linking through the dispatcher should succeed");

        assert_eq!(
            linked_roles(&pool, &file).await,
            vec![FileRole::Attachment],
            "the Link variant must reach link, not merely return Ok"
        );
    }

    #[tokio::test]
    async fn the_unlink_variant_reaches_unlink_and_removes_the_association() {
        let pool = pool().await;
        let content = seed_content(&pool).await;
        let file = seed_file(&pool).await;

        dispatch(
            &pool,
            ContentFilesCommands::Link(link::LinkArgs {
                file: file.clone(),
                content: content.as_str().to_owned(),
                role: link::FileRoleArg::Attachment,
                order: 0,
            }),
        )
        .await
        .expect("seed the link");

        dispatch(
            &pool,
            ContentFilesCommands::Unlink(unlink::UnlinkArgs {
                file: file.clone(),
                content: content.as_str().to_owned(),
                yes: true,
                dry_run: false,
            }),
        )
        .await
        .expect("unlinking through the dispatcher should succeed");

        assert!(
            linked_roles(&pool, &file).await.is_empty(),
            "the Unlink variant must actually remove the association"
        );
    }

    // Why: a dry run is the one path that must change nothing. Routed to the
    // wrong arm, or with the flag dropped, it would delete the association it
    // was asked to preview.
    #[tokio::test]
    async fn a_dry_run_unlink_leaves_the_association_in_place() {
        let pool = pool().await;
        let content = seed_content(&pool).await;
        let file = seed_file(&pool).await;

        dispatch(
            &pool,
            ContentFilesCommands::Link(link::LinkArgs {
                file: file.clone(),
                content: content.as_str().to_owned(),
                role: link::FileRoleArg::Inline,
                order: 0,
            }),
        )
        .await
        .expect("seed the link");

        dispatch(
            &pool,
            ContentFilesCommands::Unlink(unlink::UnlinkArgs {
                file: file.clone(),
                content: content.as_str().to_owned(),
                yes: true,
                dry_run: true,
            }),
        )
        .await
        .expect("a dry run should succeed");

        assert_eq!(
            linked_roles(&pool, &file).await,
            vec![FileRole::Inline],
            "a previewed unlink must not have unlinked anything"
        );
    }

    #[tokio::test]
    async fn the_list_variant_reaches_list_rather_than_erroring() {
        let pool = pool().await;
        let content = seed_content(&pool).await;

        dispatch(
            &pool,
            ContentFilesCommands::List(list::ListArgs {
                content: Some(content.as_str().to_owned()),
                file: None,
            }),
        )
        .await
        .expect("listing a content item with no files is not a failure");
    }

    async fn link(pool: &DbPool, file: &str, content: &str, role: link::FileRoleArg) {
        dispatch(
            pool,
            ContentFilesCommands::Link(link::LinkArgs {
                file: file.to_owned(),
                content: content.to_owned(),
                role,
                order: 0,
            }),
        )
        .await
        .expect("seed the link");
    }

    #[tokio::test]
    async fn the_featured_variant_reaches_featured_and_promotes_the_link() {
        let pool = pool().await;
        let content = seed_content(&pool).await;
        let file = seed_file(&pool).await;
        link(
            &pool,
            &file,
            content.as_str(),
            link::FileRoleArg::Attachment,
        )
        .await;

        dispatch(
            &pool,
            ContentFilesCommands::Featured(featured::FeaturedArgs {
                content: content.as_str().to_owned(),
                set: Some(file.clone()),
            }),
        )
        .await
        .expect("featuring a linked file should succeed");

        assert_eq!(
            linked_roles(&pool, &file).await,
            vec![FileRole::Featured],
            "the Featured variant must reach featured and promote the existing link"
        );
    }

    // Why: featuring promotes an existing link rather than creating one. A file
    // that was never attached must be refused, or content would carry a
    // featured image it has no relationship to.
    #[tokio::test]
    async fn featuring_a_file_that_is_not_linked_is_refused() {
        let pool = pool().await;
        let content = seed_content(&pool).await;
        let file = seed_file(&pool).await;

        let err = dispatch(
            &pool,
            ContentFilesCommands::Featured(featured::FeaturedArgs {
                content: content.as_str().to_owned(),
                set: Some(file.clone()),
            }),
        )
        .await
        .expect_err("an unlinked file must not become the featured image");

        assert!(
            format!("{err:#}").contains(&file),
            "the refusal should name the file: {err:#}"
        );
        assert!(linked_roles(&pool, &file).await.is_empty());
    }

    // Why: content has one featured image. Promoting a second must demote the
    // first in the same transaction, or a later read picks whichever row it
    // happens to see.
    #[tokio::test]
    async fn featuring_a_second_file_demotes_the_first() {
        let pool = pool().await;
        let content = seed_content(&pool).await;
        let first = seed_file(&pool).await;
        let second = seed_file(&pool).await;
        link(
            &pool,
            &first,
            content.as_str(),
            link::FileRoleArg::Attachment,
        )
        .await;
        link(
            &pool,
            &second,
            content.as_str(),
            link::FileRoleArg::Attachment,
        )
        .await;

        for file in [&first, &second] {
            dispatch(
                &pool,
                ContentFilesCommands::Featured(featured::FeaturedArgs {
                    content: content.as_str().to_owned(),
                    set: Some(file.clone()),
                }),
            )
            .await
            .expect("featuring should succeed");
        }

        assert_eq!(
            linked_roles(&pool, &second).await,
            vec![FileRole::Featured],
            "the most recently featured file holds the role"
        );
        assert_eq!(
            linked_roles(&pool, &first).await,
            vec![FileRole::Attachment],
            "the previous featured image must be demoted, not left as a second one"
        );
    }
}
