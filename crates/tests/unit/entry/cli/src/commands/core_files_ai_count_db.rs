//! `core files ai count` — how many AI-generated images exist.
//!
//! The `--user` filter is the interesting part: without it the count is
//! global, so a filtered count that quietly ignored its filter would report
//! one operator the size of everyone else's library.
//!
//! Every assertion is scoped to a freshly-generated user id. The unfiltered
//! count reads every row in the shared test database, so it is asserted as a
//! lower bound rather than an exact figure — an exact one would race any
//! concurrent test that inserts a file.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::core::files::ai::count::{CountArgs, execute};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{
    fixture_app_context, fixture_database_url, fixture_db_pool, seed_user_row,
};
use uuid::Uuid;

async fn pool() -> DbPool {
    fixture_db_pool(&fixture_database_url().expect("DATABASE_URL"))
        .await
        .expect("the ai count tests need a reachable test database")
}

fn ctx(pool: &DbPool) -> CommandContext {
    let url = fixture_database_url().expect("DATABASE_URL");
    CommandContext::with_app_context(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        fixture_app_context(pool, &url).expect("app context"),
    )
}

async fn seeded_user(pool: &DbPool) -> String {
    let id = format!("aicount-{}", Uuid::new_v4().simple());
    seed_user_row(pool, &UserId::new(&id), &format!("{id}@aicount.invalid"))
        .await
        .expect("seed user");
    id
}

struct File<'a> {
    user: Option<&'a str>,
    ai: bool,
    deleted: bool,
}

fn cfg_json() -> CliConfig {
    CliConfig::new()
        .with_interactive(false)
        .with_output_format(OutputFormat::Json)
}

async fn seed_file(pool: &DbPool, f: &File<'_>) -> String {
    let id = Uuid::new_v4();
    let path = format!("/tmp/aicount/{}.png", Uuid::new_v4());
    let write = pool.write_pool_arc().expect("write pool");
    sqlx::query(
        "INSERT INTO files (id, path, public_url, mime_type, ai_content, user_id, deleted_at) \
         VALUES ($5, $1, $1, 'image/png', $2, $3, CASE WHEN $4 THEN NOW() ELSE NULL END)",
    )
    .bind(&path)
    .bind(f.ai)
    .bind(f.user)
    .bind(f.deleted)
    .bind(id)
    .execute(&*write)
    .await
    .expect("seed file");
    id.to_string()
}

async fn count_for(pool: &DbPool, user: Option<&str>) -> i64 {
    let output = execute(
        CountArgs {
            user: user.map(str::to_owned),
        },
        &ctx(pool),
    )
    .await
    .expect("counting should succeed");

    let artifact = serde_json::to_value(output.artifact()).expect("serialise artifact");
    artifact["sections"]
        .as_array()
        .and_then(|sections| {
            sections
                .iter()
                .find(|s| s["heading"] == "count")
                .and_then(|s| s["content"].as_i64())
        })
        .unwrap_or_else(|| panic!("no count section in artifact: {artifact}"))
}

// Why: this is the filter's whole job. Counting everyone's files under one
// user's name would report a number that is not theirs.
#[tokio::test]
async fn a_user_filter_counts_only_that_users_images() {
    let pool = pool().await;
    let mine = seeded_user(&pool).await;
    let theirs = seeded_user(&pool).await;

    for _ in 0..3 {
        seed_file(
            &pool,
            &File {
                user: Some(&mine),
                ai: true,
                deleted: false,
            },
        )
        .await;
    }
    seed_file(
        &pool,
        &File {
            user: Some(&theirs),
            ai: true,
            deleted: false,
        },
    )
    .await;

    assert_eq!(
        count_for(&pool, Some(&mine)).await,
        3,
        "another user's images must not be counted here"
    );
    assert_eq!(count_for(&pool, Some(&theirs)).await, 1);
}

// Why: the command counts AI images specifically. Counting every upload would
// silently inflate the figure an operator uses to judge generation volume.
#[tokio::test]
async fn files_that_are_not_ai_generated_are_not_counted() {
    let pool = pool().await;
    let user = seeded_user(&pool).await;

    seed_file(
        &pool,
        &File {
            user: Some(&user),
            ai: true,
            deleted: false,
        },
    )
    .await;
    seed_file(
        &pool,
        &File {
            user: Some(&user),
            ai: false,
            deleted: false,
        },
    )
    .await;

    assert_eq!(
        count_for(&pool, Some(&user)).await,
        1,
        "a plain upload is not an AI image"
    );
}

// Why: deletion is soft, so the row survives. A count that ignored
// `deleted_at` would keep reporting images the user has already removed.
#[tokio::test]
async fn soft_deleted_images_stop_being_counted() {
    let pool = pool().await;
    let user = seeded_user(&pool).await;

    seed_file(
        &pool,
        &File {
            user: Some(&user),
            ai: true,
            deleted: false,
        },
    )
    .await;
    seed_file(
        &pool,
        &File {
            user: Some(&user),
            ai: true,
            deleted: true,
        },
    )
    .await;

    assert_eq!(
        count_for(&pool, Some(&user)).await,
        1,
        "a deleted image must leave the count"
    );
}

#[tokio::test]
async fn a_user_with_no_images_counts_zero_rather_than_failing() {
    let pool = pool().await;
    let user = seeded_user(&pool).await;

    assert_eq!(count_for(&pool, Some(&user)).await, 0);
}

// Why: without a filter the count spans every user. Asserted as a lower bound
// because the shared database carries other tests' rows.
#[tokio::test]
async fn an_unfiltered_count_spans_users_rather_than_scoping_to_one() {
    let pool = pool().await;
    let mine = seeded_user(&pool).await;
    let theirs = seeded_user(&pool).await;

    for user in [&mine, &theirs] {
        seed_file(
            &pool,
            &File {
                user: Some(user),
                ai: true,
                deleted: false,
            },
        )
        .await;
    }

    let global = count_for(&pool, None).await;
    assert!(
        global >= 2,
        "an unfiltered count must include both users' images, got {global}"
    );
    assert!(
        global > count_for(&pool, Some(&mine)).await,
        "the unfiltered count must exceed a single user's"
    );
}

// `core files ai show` and `list` reach the database through a pool seam, so
// these drive the seam directly. The guard worth asserting is that `ai show`
// refuses a file that is not AI-generated: without it the command is just
// `files show` under a name that promises otherwise, and an operator filtering
// for generated content sees uploads.
mod show_and_list {
    use super::{File, cfg_json, pool, seed_file, seeded_user};
    use systemprompt_cli::core::files::ai::list::{ListArgs, execute_with_pool as list_with_pool};
    use systemprompt_cli::core::files::ai::show::{ShowArgs, execute_with_pool as show_with_pool};
    use systemprompt_database::DbPool;
    use uuid::Uuid;

    async fn ai_file(pool: &DbPool, user: &str) -> String {
        seed_file(
            pool,
            &File {
                user: Some(user),
                ai: true,
                deleted: false,
            },
        )
        .await
    }

    async fn show(pool: &DbPool, id: &str) -> anyhow::Result<serde_json::Value> {
        let out = show_with_pool(
            ShowArgs {
                file: id.to_owned(),
            },
            pool,
            &cfg_json(),
        )
        .await?;
        Ok(serde_json::to_value(out.artifact()).expect("serialise artifact"))
    }

    async fn list_ids(pool: &DbPool, user: Option<&str>) -> Vec<String> {
        let out = list_with_pool(
            ListArgs {
                limit: 100,
                offset: 0,
                user: user.map(str::to_owned),
            },
            pool,
            &cfg_json(),
        )
        .await
        .expect("listing should succeed");

        serde_json::to_value(out.artifact()).expect("serialise artifact")["items"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|row| row["id"].as_str().map(str::to_owned))
            .collect()
    }

    #[tokio::test]
    async fn an_ai_image_is_shown() {
        let pool = pool().await;
        let user = seeded_user(&pool).await;
        let id = ai_file(&pool, &user).await;

        let artifact = show(&pool, &id).await.expect("an AI image should be shown");

        assert!(
            serde_json::to_string(&artifact)
                .expect("stringify")
                .contains(&id),
            "the report should name the file it was asked for"
        );
    }

    // Why: the command promises AI-generated images. Showing an ordinary
    // upload here means an operator auditing generated content is shown files
    // that were never generated.
    #[tokio::test]
    async fn a_file_that_is_not_ai_generated_is_refused() {
        let pool = pool().await;
        let user = seeded_user(&pool).await;
        let plain = seed_file(
            &pool,
            &File {
                user: Some(&user),
                ai: false,
                deleted: false,
            },
        )
        .await;

        let err = show(&pool, &plain)
            .await
            .expect_err("a plain upload is not an AI image");

        assert!(
            format!("{err:#}").contains("not an AI-generated image"),
            "the refusal should say why: {err:#}"
        );
    }

    #[tokio::test]
    async fn an_unknown_file_is_reported_as_not_found() {
        let pool = pool().await;

        let err = show(&pool, &Uuid::new_v4().to_string())
            .await
            .expect_err("an unknown id must not render as an empty report");

        assert!(format!("{err:#}").contains("File not found"), "{err:#}");
    }

    // Why: a malformed id must be refused at the boundary rather than reaching
    // the repository, where it would be a database error rather than an answer
    // an operator can act on.
    #[tokio::test]
    async fn a_file_id_that_is_not_a_uuid_is_refused_before_the_lookup() {
        let pool = pool().await;

        let err = show(&pool, "not-a-uuid")
            .await
            .expect_err("a malformed id is not a lookup");

        assert!(!format!("{err:#}").is_empty());
    }

    // Why: the same scoping as the count. A listing that ignored `--user`
    // would show one operator another's generated images.
    #[tokio::test]
    async fn a_user_filter_lists_only_that_users_images() {
        let pool = pool().await;
        let mine = seeded_user(&pool).await;
        let theirs = seeded_user(&pool).await;
        let ours = ai_file(&pool, &mine).await;
        let other = ai_file(&pool, &theirs).await;

        let listed = list_ids(&pool, Some(&mine)).await;

        assert!(listed.contains(&ours));
        assert!(
            !listed.contains(&other),
            "another user's images must not appear: {listed:?}"
        );
    }

    // Why: deletion is soft, so the row survives. A listing that ignored
    // `deleted_at` keeps offering images the user has removed.
    #[tokio::test]
    async fn soft_deleted_images_are_not_listed() {
        let pool = pool().await;
        let user = seeded_user(&pool).await;
        let removed = seed_file(
            &pool,
            &File {
                user: Some(&user),
                ai: true,
                deleted: true,
            },
        )
        .await;

        assert!(!list_ids(&pool, Some(&user)).await.contains(&removed));
    }
}
