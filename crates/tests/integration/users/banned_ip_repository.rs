//! Integration tests for BannedIpRepository.
//!
//! Tests cover:
//! - Banning IPs with various durations
//! - Checking if an IP is banned
//! - Listing active bans
//! - Unbanning IPs
//! - Cleanup of expired bans

use anyhow::Result;
use systemprompt_core_database::Database;
use systemprompt_core_users::{
    BanDuration, BanIpParams, BanIpWithMetadataParams, BannedIpRepository,
};

async fn get_db() -> Option<Database> {
    let database_url = std::env::var("DATABASE_URL").ok()?;
    Database::new_postgres(&database_url).await.ok()
}

async fn cleanup_test_ip(repo: &BannedIpRepository, ip: &str) {
    let _ = repo.unban_ip(ip).await;
}

// ============================================================================
// BannedIpRepository::new Tests
// ============================================================================

#[tokio::test]
async fn repository_creation_from_db_pool() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = BannedIpRepository::new(&db_pool)?;

    // Verify the repo is functional by calling a method
    let count = repo.count_active_bans().await?;
    assert!(count >= 0);

    Ok(())
}

#[tokio::test]
async fn repository_creation_from_pool_arc() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let pool = db.pool_arc()?;
    let repo = BannedIpRepository::from_pool(pool);

    // Verify the repo is functional
    let count = repo.count_active_bans().await?;
    assert!(count >= 0);

    Ok(())
}

// ============================================================================
// BannedIpRepository::is_banned Tests
// ============================================================================

#[tokio::test]
async fn is_banned_returns_false_for_unbanned_ip() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = BannedIpRepository::new(&db_pool)?;

    // Use a unique IP that's unlikely to be in the database
    let test_ip = "192.168.255.254";
    cleanup_test_ip(&repo, test_ip).await;

    let is_banned = repo.is_banned(test_ip).await?;
    assert!(!is_banned);

    Ok(())
}

#[tokio::test]
async fn is_banned_returns_true_for_banned_ip() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = BannedIpRepository::new(&db_pool)?;

    let test_ip = "192.168.100.1";
    cleanup_test_ip(&repo, test_ip).await;

    // Ban the IP
    let params = BanIpParams::new(test_ip, "Test ban", BanDuration::Hours(1), "integration_test");
    repo.ban_ip(params).await?;

    let is_banned = repo.is_banned(test_ip).await?;
    assert!(is_banned);

    // Cleanup
    cleanup_test_ip(&repo, test_ip).await;

    Ok(())
}

// ============================================================================
// BannedIpRepository::ban_ip Tests
// ============================================================================

#[tokio::test]
async fn ban_ip_creates_new_ban() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = BannedIpRepository::new(&db_pool)?;

    let test_ip = "192.168.100.2";
    cleanup_test_ip(&repo, test_ip).await;

    let params = BanIpParams::new(
        test_ip,
        "Test ban creation",
        BanDuration::Days(1),
        "integration_test",
    );
    repo.ban_ip(params).await?;

    let ban = repo.get_ban(test_ip).await?;
    assert!(ban.is_some());

    let ban = ban.expect("Ban should exist");
    assert_eq!(ban.ip_address, test_ip);
    assert_eq!(ban.reason, "Test ban creation");
    assert!(!ban.is_permanent);

    // Cleanup
    cleanup_test_ip(&repo, test_ip).await;

    Ok(())
}

#[tokio::test]
async fn ban_ip_with_fingerprint() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = BannedIpRepository::new(&db_pool)?;

    let test_ip = "192.168.100.3";
    let fingerprint = "test-fingerprint-123";
    cleanup_test_ip(&repo, test_ip).await;

    let params = BanIpParams::new(test_ip, "Test ban", BanDuration::Hours(2), "integration_test")
        .with_source_fingerprint(fingerprint);
    repo.ban_ip(params).await?;

    let ban = repo.get_ban(test_ip).await?;
    assert!(ban.is_some());

    let ban = ban.expect("Ban should exist");
    assert_eq!(ban.source_fingerprint.as_deref(), Some(fingerprint));

    // Cleanup
    cleanup_test_ip(&repo, test_ip).await;

    Ok(())
}

#[tokio::test]
async fn ban_ip_permanent() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = BannedIpRepository::new(&db_pool)?;

    let test_ip = "192.168.100.4";
    cleanup_test_ip(&repo, test_ip).await;

    let params = BanIpParams::new(
        test_ip,
        "Permanent ban test",
        BanDuration::Permanent,
        "integration_test",
    );
    repo.ban_ip(params).await?;

    let ban = repo.get_ban(test_ip).await?;
    assert!(ban.is_some());

    let ban = ban.expect("Ban should exist");
    assert!(ban.is_permanent);
    assert!(ban.expires_at.is_none());

    // Cleanup
    cleanup_test_ip(&repo, test_ip).await;

    Ok(())
}

#[tokio::test]
async fn ban_ip_increments_ban_count_on_repeat() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = BannedIpRepository::new(&db_pool)?;

    let test_ip = "192.168.100.5";
    cleanup_test_ip(&repo, test_ip).await;

    // First ban
    let params = BanIpParams::new(test_ip, "First ban", BanDuration::Hours(1), "integration_test");
    repo.ban_ip(params).await?;

    let ban = repo.get_ban(test_ip).await?;
    let first_count = ban.map(|b| b.ban_count).unwrap_or(0);

    // Second ban
    let params =
        BanIpParams::new(test_ip, "Second ban", BanDuration::Hours(1), "integration_test");
    repo.ban_ip(params).await?;

    let ban = repo.get_ban(test_ip).await?;
    let second_count = ban.map(|b| b.ban_count).unwrap_or(0);

    assert_eq!(second_count, first_count + 1);

    // Cleanup
    cleanup_test_ip(&repo, test_ip).await;

    Ok(())
}

// ============================================================================
// BannedIpRepository::ban_ip_with_metadata Tests
// ============================================================================

#[tokio::test]
async fn ban_ip_with_metadata_includes_all_fields() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = BannedIpRepository::new(&db_pool)?;

    let test_ip = "192.168.100.6";
    cleanup_test_ip(&repo, test_ip).await;

    let params = BanIpWithMetadataParams::new(
        test_ip,
        "Metadata ban test",
        BanDuration::Hours(3),
        "integration_test",
    )
    .with_source_fingerprint("fp-123")
    .with_offense_path("/api/v1/malicious")
    .with_user_agent("TestBot/1.0")
    .with_session_id("session-xyz");

    repo.ban_ip_with_metadata(params).await?;

    let ban = repo.get_ban(test_ip).await?;
    assert!(ban.is_some());

    let ban = ban.expect("Ban should exist");
    assert_eq!(ban.source_fingerprint.as_deref(), Some("fp-123"));
    assert_eq!(ban.last_offense_path.as_deref(), Some("/api/v1/malicious"));
    assert_eq!(ban.last_user_agent.as_deref(), Some("TestBot/1.0"));
    assert!(ban.associated_session_ids.is_some());

    // Cleanup
    cleanup_test_ip(&repo, test_ip).await;

    Ok(())
}

// ============================================================================
// BannedIpRepository::unban_ip Tests
// ============================================================================

#[tokio::test]
async fn unban_ip_removes_ban() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = BannedIpRepository::new(&db_pool)?;

    let test_ip = "192.168.100.7";
    cleanup_test_ip(&repo, test_ip).await;

    // Ban first
    let params = BanIpParams::new(test_ip, "To be unbanned", BanDuration::Hours(1), "integration_test");
    repo.ban_ip(params).await?;

    assert!(repo.is_banned(test_ip).await?);

    // Unban
    let removed = repo.unban_ip(test_ip).await?;
    assert!(removed);

    assert!(!repo.is_banned(test_ip).await?);

    Ok(())
}

#[tokio::test]
async fn unban_ip_returns_false_for_nonexistent() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = BannedIpRepository::new(&db_pool)?;

    let test_ip = "192.168.200.200";
    cleanup_test_ip(&repo, test_ip).await;

    let removed = repo.unban_ip(test_ip).await?;
    assert!(!removed);

    Ok(())
}

// ============================================================================
// BannedIpRepository::get_ban Tests
// ============================================================================

#[tokio::test]
async fn get_ban_returns_none_for_unbanned() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = BannedIpRepository::new(&db_pool)?;

    let test_ip = "192.168.200.201";
    cleanup_test_ip(&repo, test_ip).await;

    let ban = repo.get_ban(test_ip).await?;
    assert!(ban.is_none());

    Ok(())
}

// ============================================================================
// BannedIpRepository::list_active_bans Tests
// ============================================================================

#[tokio::test]
async fn list_active_bans_returns_active_bans() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = BannedIpRepository::new(&db_pool)?;

    let test_ip = "192.168.100.8";
    cleanup_test_ip(&repo, test_ip).await;

    // Create a ban
    let params = BanIpParams::new(test_ip, "List test", BanDuration::Hours(1), "integration_test");
    repo.ban_ip(params).await?;

    let bans = repo.list_active_bans(100).await?;
    let found = bans.iter().any(|b| b.ip_address == test_ip);
    assert!(found);

    // Cleanup
    cleanup_test_ip(&repo, test_ip).await;

    Ok(())
}

// ============================================================================
// BannedIpRepository::list_bans_by_source Tests
// ============================================================================

#[tokio::test]
async fn list_bans_by_source_filters_correctly() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = BannedIpRepository::new(&db_pool)?;

    let test_ip = "192.168.100.9";
    let unique_source = "unique_integration_test_source";
    cleanup_test_ip(&repo, test_ip).await;

    // Create a ban with unique source
    let params = BanIpParams::new(test_ip, "Source test", BanDuration::Hours(1), unique_source);
    repo.ban_ip(params).await?;

    let bans = repo.list_bans_by_source(unique_source, 100).await?;
    assert!(!bans.is_empty());
    assert!(bans.iter().all(|b| b.ban_source.as_deref() == Some(unique_source)));

    // Cleanup
    cleanup_test_ip(&repo, test_ip).await;

    Ok(())
}

// ============================================================================
// BannedIpRepository::list_bans_by_fingerprint Tests
// ============================================================================

#[tokio::test]
async fn list_bans_by_fingerprint_filters_correctly() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = BannedIpRepository::new(&db_pool)?;

    let test_ip = "192.168.100.10";
    let unique_fp = "unique_test_fingerprint_xyz";
    cleanup_test_ip(&repo, test_ip).await;

    // Create a ban with unique fingerprint
    let params = BanIpParams::new(test_ip, "FP test", BanDuration::Hours(1), "integration_test")
        .with_source_fingerprint(unique_fp);
    repo.ban_ip(params).await?;

    let bans = repo.list_bans_by_fingerprint(unique_fp).await?;
    assert!(!bans.is_empty());
    assert!(bans.iter().all(|b| b.source_fingerprint.as_deref() == Some(unique_fp)));

    // Cleanup
    cleanup_test_ip(&repo, test_ip).await;

    Ok(())
}

// ============================================================================
// BannedIpRepository::count_active_bans Tests
// ============================================================================

#[tokio::test]
async fn count_active_bans_returns_count() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = BannedIpRepository::new(&db_pool)?;

    let count = repo.count_active_bans().await?;
    assert!(count >= 0);

    Ok(())
}

// ============================================================================
// BannedIpRepository::cleanup_expired Tests
// ============================================================================

#[tokio::test]
async fn cleanup_expired_runs_without_error() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = BannedIpRepository::new(&db_pool)?;

    // Just verify the method runs without error
    let deleted = repo.cleanup_expired().await?;
    assert!(deleted >= 0);

    Ok(())
}
