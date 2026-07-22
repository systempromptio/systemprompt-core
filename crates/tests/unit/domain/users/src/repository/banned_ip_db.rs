//! DB-backed tests for banned-IP mutation, lookup, and listing queries.

use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use systemprompt_users::{BanDuration, BanIpParams, BanIpWithMetadataParams, BannedIpRepository};
use uuid::Uuid;

struct Ctx {
    repo: BannedIpRepository,
    ip: String,
    source: String,
    fingerprint: String,
}

async fn setup(prefix: &str) -> Option<Ctx> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = BannedIpRepository::new(&pool).expect("repo");
    let tag = Uuid::new_v4();
    let octet = u128::from_le_bytes(*tag.as_bytes()) % 200 + 10;
    Some(Ctx {
        repo,
        ip: format!("10.{}.{}.{}", octet % 250, (octet / 7) % 250, prefix.len()),
        source: format!("src-{prefix}-{}", tag.simple()),
        fingerprint: format!("fp-{prefix}-{}", tag.simple()),
    })
}

#[tokio::test]
async fn ban_then_query_then_unban_round_trip() {
    let Some(ctx) = setup("round").await else {
        return;
    };
    let params = BanIpParams::new(&ctx.ip, "abuse", BanDuration::Hours(2), &ctx.source)
        .with_source_fingerprint(&ctx.fingerprint);
    ctx.repo.ban_ip(params).await.expect("ban");

    assert!(ctx.repo.is_banned(&ctx.ip).await.expect("is_banned"));
    let ban = ctx
        .repo
        .find_ban(&ctx.ip)
        .await
        .expect("find")
        .expect("row");
    assert_eq!(ban.reason, "abuse");
    assert_eq!(ban.ban_count, 1);
    assert!(!ban.is_permanent);
    assert!(ban.expires_at.is_some());
    assert_eq!(
        ban.source_fingerprint.as_deref(),
        Some(ctx.fingerprint.as_str())
    );

    assert!(ctx.repo.count_active_bans().await.expect("count") >= 1);
    let active = ctx.repo.list_active_bans(1_000).await.expect("active");
    assert!(active.iter().any(|b| b.ip_address == ctx.ip));
    let by_source = ctx
        .repo
        .list_bans_by_source(&ctx.source, 100)
        .await
        .expect("by source");
    assert_eq!(by_source.len(), 1);
    let by_fp = ctx
        .repo
        .list_bans_by_fingerprint(&ctx.fingerprint)
        .await
        .expect("by fp");
    assert_eq!(by_fp.len(), 1);

    assert!(ctx.repo.unban_ip(&ctx.ip).await.expect("unban"));
    assert!(!ctx.repo.unban_ip(&ctx.ip).await.expect("second unban"));
    assert!(!ctx.repo.is_banned(&ctx.ip).await.expect("cleared"));
}

#[tokio::test]
async fn reban_increments_count_and_permanent_ban_stays_permanent() {
    let Some(ctx) = setup("perm").await else {
        return;
    };
    ctx.repo
        .ban_ip(BanIpParams::new(
            &ctx.ip,
            "first",
            BanDuration::Permanent,
            &ctx.source,
        ))
        .await
        .expect("permanent ban");

    ctx.repo
        .ban_ip(BanIpParams::new(
            &ctx.ip,
            "second",
            BanDuration::Hours(1),
            &ctx.source,
        ))
        .await
        .expect("re-ban");

    let ban = ctx
        .repo
        .find_ban(&ctx.ip)
        .await
        .expect("find")
        .expect("row");
    assert_eq!(ban.ban_count, 2);
    assert_eq!(ban.reason, "second");
    assert!(ban.is_permanent, "permanent flag must survive re-ban");
    assert!(ban.expires_at.is_none(), "permanent expiry must not shrink");

    ctx.repo.unban_ip(&ctx.ip).await.expect("cleanup");
}

#[tokio::test]
async fn ban_with_metadata_accumulates_session_ids_and_keeps_metadata() {
    let Some(ctx) = setup("meta").await else {
        return;
    };
    let first = BanIpWithMetadataParams::new(&ctx.ip, "bot", BanDuration::Days(1), &ctx.source)
        .with_source_fingerprint(&ctx.fingerprint)
        .with_offense_path("/admin")
        .with_user_agent("badbot/1.0")
        .with_session_id("sess-one");
    ctx.repo.ban_ip_with_metadata(first).await.expect("first");

    let second =
        BanIpWithMetadataParams::new(&ctx.ip, "bot again", BanDuration::Days(1), &ctx.source)
            .with_session_id("sess-two");
    ctx.repo.ban_ip_with_metadata(second).await.expect("second");

    let ban = ctx
        .repo
        .find_ban(&ctx.ip)
        .await
        .expect("find")
        .expect("row");
    assert_eq!(ban.ban_count, 2);
    assert_eq!(ban.last_offense_path.as_deref(), Some("/admin"));
    assert_eq!(ban.last_user_agent.as_deref(), Some("badbot/1.0"));
    assert_eq!(
        ban.source_fingerprint.as_deref(),
        Some(ctx.fingerprint.as_str())
    );
    let sessions = ban.associated_session_ids.expect("session ids");
    assert_eq!(sessions, vec!["sess-one".to_owned(), "sess-two".to_owned()]);

    ctx.repo.unban_ip(&ctx.ip).await.expect("cleanup");
}

#[tokio::test]
async fn expired_ban_is_invisible_and_removed_by_cleanup() {
    let Some(ctx) = setup("exp").await else {
        return;
    };
    ctx.repo
        .ban_ip(
            BanIpParams::new(&ctx.ip, "expired", BanDuration::Hours(-2), &ctx.source)
                .with_source_fingerprint(&ctx.fingerprint),
        )
        .await
        .expect("expired ban");

    assert!(!ctx.repo.is_banned(&ctx.ip).await.expect("is_banned"));
    assert!(ctx.repo.find_ban(&ctx.ip).await.expect("find").is_none());

    ctx.repo.cleanup_expired().await.expect("cleanup");
    let remaining = ctx
        .repo
        .list_bans_by_fingerprint(&ctx.fingerprint)
        .await
        .expect("by fp");
    assert!(remaining.is_empty());
}
