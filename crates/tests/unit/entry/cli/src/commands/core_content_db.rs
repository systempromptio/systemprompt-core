//! DB-backed tests for the `core content` command family.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::core::content::{
    delete, edit, link, list, popular, search, show, status, verify,
};
use systemprompt_cli::interactive::ScriptedPrompter;
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides};
use systemprompt_content::models::CreateContentParams;
use systemprompt_content::{Content, ContentRepository};
use systemprompt_database::DbPool;
use systemprompt_identifiers::SourceId;
use systemprompt_runtime::DatabaseContext;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

async fn pool() -> DbPool {
    fixture_db_pool(&fixture_database_url().unwrap())
        .await
        .unwrap()
}


fn card_title(out: &systemprompt_cli::shared::CommandOutput) -> String {
    serde_json::to_value(out.artifact())
        .ok()
        .and_then(|v| v.get("title").and_then(|t| t.as_str()).map(str::to_owned))
        .unwrap_or_default()
}

fn cfg() -> CliConfig {
    CliConfig::new().with_interactive(false)
}

fn unique(prefix: &str) -> String {
    format!("{prefix}-{}", uuid::Uuid::new_v4())
}

fn slug() -> String {
    format!("slug{}", uuid::Uuid::new_v4().simple())
}

async fn seed_content(pool: &DbPool, source: &str, slug: &str) -> Content {
    let repo = ContentRepository::new(pool).unwrap();
    let params = CreateContentParams::new(
        slug.to_owned(),
        format!("Title for {slug}"),
        "A description".to_owned(),
        "The body".to_owned(),
        SourceId::new(source.to_owned()),
    )
    .with_keywords("alpha,beta".to_owned())
    .with_version_hash("hash-1".to_owned());
    repo.create(&params).await.unwrap()
}

fn db_ctx(pool: &DbPool, config: CliConfig) -> CommandContext {
    CommandContext::with_database(
        config,
        EnvOverrides::default(),
        DatabaseContext::from_pool(pool.clone()),
        fixture_database_url().unwrap(),
    )
}

fn edit_args(identifier: Option<String>) -> edit::EditArgs {
    edit::EditArgs {
        identifier,
        source: None,
        set_values: vec![],
        public: false,
        private: false,
        body: None,
        body_file: None,
    }
}

#[tokio::test]
async fn edit_requires_identifier_in_non_interactive_mode() {
    let pool = pool().await;
    let prompter = ScriptedPrompter::new(Vec::<String>::new());
    let err = edit::execute_with_pool(edit_args(None), &prompter, &pool, &cfg())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("--identifier is required"));
}

#[tokio::test]
async fn edit_unknown_content_id_errors() {
    let pool = pool().await;
    let prompter = ScriptedPrompter::new(Vec::<String>::new());
    let args = edit_args(Some(format!("content_{}", uuid::Uuid::new_v4())));
    let err = edit::execute_with_pool(args, &prompter, &pool, &cfg())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Content not found"));
}

#[tokio::test]
async fn edit_slug_without_source_errors() {
    let pool = pool().await;
    let prompter = ScriptedPrompter::new(Vec::<String>::new());
    let err = edit::execute_with_pool(
        edit_args(Some("some-slug".to_owned())),
        &prompter,
        &pool,
        &cfg(),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("Source ID required"));
}

#[tokio::test]
async fn edit_without_changes_errors() {
    let pool = pool().await;
    let source = unique("src");
    let content = seed_content(&pool, &source, &slug()).await;
    let prompter = ScriptedPrompter::new(Vec::<String>::new());
    let err = edit::execute_with_pool(
        edit_args(Some(content.id.as_str().to_owned())),
        &prompter,
        &pool,
        &cfg(),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("No changes specified"));
}

#[tokio::test]
async fn edit_sets_scalar_fields() {
    let pool = pool().await;
    let source = unique("src");
    let content = seed_content(&pool, &source, &slug()).await;
    let mut args = edit_args(Some(content.id.as_str().to_owned()));
    args.set_values = vec![
        "title=New Title".to_owned(),
        "description=New Desc".to_owned(),
        "keywords=x,y".to_owned(),
        "image=cover.png".to_owned(),
        "kind=guide".to_owned(),
        "public=false".to_owned(),
    ];
    let prompter = ScriptedPrompter::new(Vec::<String>::new());
    let out = edit::execute_with_pool(args, &prompter, &pool, &cfg())
        .await
        .unwrap();
    assert_eq!(card_title(&out), "Content Updated");

    let repo = ContentRepository::new(&pool).unwrap();
    let updated = repo.get_by_id(&content.id).await.unwrap().unwrap();
    assert_eq!(updated.title, "New Title");
    assert_eq!(updated.description, "New Desc");
    assert_eq!(updated.keywords, "x,y");
    assert_eq!(updated.image.as_deref(), Some("cover.png"));
    assert_eq!(updated.kind, "guide");
    assert!(!updated.public);
}

#[tokio::test]
async fn edit_by_slug_with_source_and_flags() {
    let pool = pool().await;
    let source = unique("src");
    let slug = slug();
    seed_content(&pool, &source, &slug).await;
    let mut args = edit_args(Some(slug.clone()));
    args.source = Some(source.clone());
    args.private = true;
    args.body = Some("edited body".to_owned());
    let prompter = ScriptedPrompter::new(Vec::<String>::new());
    edit::execute_with_pool(args, &prompter, &pool, &cfg())
        .await
        .unwrap();

    let repo = ContentRepository::new(&pool).unwrap();
    let updated = repo
        .get_by_source_and_slug(
            &SourceId::new(source),
            &slug,
            &systemprompt_identifiers::LocaleCode::new("en"),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.body, "edited body");
    assert!(!updated.public);
}

#[tokio::test]
async fn edit_rejects_malformed_and_unknown_set_values() {
    let pool = pool().await;
    let source = unique("src");
    let content = seed_content(&pool, &source, &slug()).await;
    let prompter = ScriptedPrompter::new(Vec::<String>::new());

    for (set_value, expected) in [
        ("titleonly", "Invalid --set format"),
        ("nope=1", "Unknown field"),
        ("kind=novel", "Invalid kind"),
        ("public=maybe", "Invalid boolean value"),
        ("category=missing-category", "not found"),
    ] {
        let mut args = edit_args(Some(content.id.as_str().to_owned()));
        args.set_values = vec![set_value.to_owned()];
        let err = edit::execute_with_pool(args, &prompter, &pool, &cfg())
            .await
            .unwrap_err();
        assert!(err.to_string().contains(expected), "{set_value}: {err}");
    }
}

#[tokio::test]
async fn edit_clears_image_and_category() {
    let pool = pool().await;
    let source = unique("src");
    let content = seed_content(&pool, &source, &slug()).await;
    let mut args = edit_args(Some(content.id.as_str().to_owned()));
    args.set_values = vec!["image=none".to_owned(), "category=none".to_owned()];
    let prompter = ScriptedPrompter::new(Vec::<String>::new());
    edit::execute_with_pool(args, &prompter, &pool, &cfg())
        .await
        .unwrap();

    let repo = ContentRepository::new(&pool).unwrap();
    let updated = repo.get_by_id(&content.id).await.unwrap().unwrap();
    assert!(updated.image.is_none());
    assert!(updated.category_id.is_none());
}

#[tokio::test]
async fn show_finds_content_by_id_and_slug() {
    let pool = pool().await;
    let source = unique("src");
    let slug = slug();
    let content = seed_content(&pool, &source, &slug).await;

    let by_id = show::execute_with_pool(
        show::ShowArgs {
            identifier: content.id.as_str().to_owned(),
            source: None,
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap();
    assert_eq!(card_title(&by_id), "Content Details");

    show::execute_with_pool(
        show::ShowArgs {
            identifier: slug,
            source: Some(source),
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn show_missing_content_errors() {
    let pool = pool().await;
    let err = show::execute_with_pool(
        show::ShowArgs {
            identifier: unique("missing-slug"),
            source: Some(unique("missing-src")),
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().to_lowercase().contains("not found"));
}

#[tokio::test]
async fn list_filters_by_source() {
    let pool = pool().await;
    let source = unique("src");
    seed_content(&pool, &source, &slug()).await;
    seed_content(&pool, &source, &slug()).await;

    let args = list::ListArgs {
        source: Some(source),
        category: None,
        limit: 20,
        offset: 0,
    };
    let out = list::execute_with_pool(args, &pool, &cfg()).await.unwrap();
    assert!(out.title().is_some());

    list::execute_with_pool(
        list::ListArgs {
            source: None,
            category: None,
            limit: 5,
            offset: 0,
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn search_runs_with_and_without_filters() {
    let pool = pool().await;
    let source = unique("src");
    seed_content(&pool, &source, &slug()).await;

    search::execute_with_pool(
        search::SearchArgs {
            query: "Title".to_owned(),
            source: Some(source),
            category: None,
            limit: 10,
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap();

    search::execute_with_pool(
        search::SearchArgs {
            query: "zzz-no-match".to_owned(),
            source: None,
            category: Some("no-such-category".to_owned()),
            limit: 10,
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn status_reports_prerender_health() {
    let pool = pool().await;
    let source = unique("src");
    seed_content(&pool, &source, &slug()).await;
    let dist = tempfile::tempdir().unwrap();

    let out = status::execute_with_pool(
        status::StatusArgs {
            source: source.clone(),
            web_dist: Some(dist.path().to_path_buf()),
            url_pattern: Some("/docs/{slug}".to_owned()),
            limit: 10,
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap();
    assert!(out.title().is_some());

    status::execute_with_pool(
        status::StatusArgs {
            source,
            web_dist: None,
            url_pattern: None,
            limit: 10,
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn popular_parses_duration_and_lists() {
    let pool = pool().await;
    let source = unique("src");
    seed_content(&pool, &source, &slug()).await;

    popular::execute_with_pool(
        popular::PopularArgs {
            source: source.clone(),
            since: "7d".to_owned(),
            limit: 5,
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap();

    let err = popular::execute_with_pool(
        popular::PopularArgs {
            source,
            since: "not-a-duration".to_owned(),
            limit: 5,
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap_err();
    assert!(!err.to_string().is_empty());
}

#[tokio::test]
async fn delete_dry_run_keeps_content() {
    let pool = pool().await;
    let source = unique("src");
    let content = seed_content(&pool, &source, &slug()).await;
    let ctx = db_ctx(&pool, cfg());

    let out = delete::execute(
        delete::DeleteArgs {
            identifier: content.id.as_str().to_owned(),
            source: None,
            yes: false,
            dry_run: true,
        },
        &ctx,
    )
    .await
    .unwrap();
    assert_eq!(card_title(&out), "Content Delete (Dry Run)");

    let repo = ContentRepository::new(&pool).unwrap();
    assert!(repo.get_by_id(&content.id).await.unwrap().is_some());
}

#[tokio::test]
async fn delete_requires_yes_in_non_interactive_mode() {
    let pool = pool().await;
    let source = unique("src");
    let content = seed_content(&pool, &source, &slug()).await;
    let ctx = db_ctx(&pool, cfg());

    let err = delete::execute(
        delete::DeleteArgs {
            identifier: content.id.as_str().to_owned(),
            source: None,
            yes: false,
            dry_run: false,
        },
        &ctx,
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("--yes is required"));
}

#[tokio::test]
async fn delete_with_yes_removes_content() {
    let pool = pool().await;
    let source = unique("src");
    let slug = slug();
    let content = seed_content(&pool, &source, &slug).await;
    let ctx = db_ctx(&pool, cfg());

    let out = delete::execute(
        delete::DeleteArgs {
            identifier: slug,
            source: Some(source),
            yes: true,
            dry_run: false,
        },
        &ctx,
    )
    .await
    .unwrap();
    assert_eq!(card_title(&out), "Content Deleted");

    let repo = ContentRepository::new(&pool).unwrap();
    assert!(repo.get_by_id(&content.id).await.unwrap().is_none());
}

#[tokio::test]
async fn verify_reports_database_and_prerender_state() {
    let pool = pool().await;
    let source = unique("src");
    let slug = slug();
    let content = seed_content(&pool, &source, &slug).await;
    let ctx = db_ctx(&pool, cfg());
    let dist = tempfile::tempdir().unwrap();

    let out = verify::execute(
        verify::VerifyArgs {
            identifier: content.id.as_str().to_owned(),
            source: None,
            web_dist: Some(dist.path().to_path_buf()),
            base_url: None,
            url_pattern: Some("/{source}/{slug}".to_owned()),
        },
        &ctx,
    )
    .await
    .unwrap();
    assert_eq!(card_title(&out), "Content Verification");

    let err = verify::execute(
        verify::VerifyArgs {
            identifier: unique("missing"),
            source: None,
            web_dist: None,
            base_url: None,
            url_pattern: None,
        },
        &ctx,
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("--source required"));
}

#[tokio::test]
async fn link_list_requires_a_filter_flag() {
    let pool = pool().await;
    let ctx = db_ctx(&pool, cfg());
    let err = link::list::execute(
        link::list::ListArgs {
            campaign: None,
            content: None,
        },
        &ctx,
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("--campaign or --content"));
}

#[tokio::test]
async fn link_list_returns_empty_for_unknown_content() {
    let pool = pool().await;
    let ctx = db_ctx(&pool, cfg());
    link::list::execute(
        link::list::ListArgs {
            campaign: None,
            content: Some(unique("content")),
        },
        &ctx,
    )
    .await
    .unwrap();
}
