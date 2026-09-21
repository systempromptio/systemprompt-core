use systemprompt_identifiers::{TaskId, UserId};
use systemprompt_marketplace::inventory::{BaselinePreparation, LatestPublicationStatus};
use systemprompt_models::profile::PathsConfig;
use systemprompt_runtime::managed::inventory;
use systemprompt_test_fixtures::{
    fixture_app_context_with, fixture_db_pool, init_isolated_bootstrap, seed_user_row,
};

fn skill(root: &std::path::Path, body: &str) {
    let dir = root.join("skills/runtime-owned");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("config.yaml"),
        "id: runtime-owned\nname: Runtime owned\ndescription: fixture\nenabled: true\nfile: SKILL.md\n",
    )
    .unwrap();
    std::fs::write(dir.join("SKILL.md"), body).unwrap();
}

#[tokio::test]
async fn runtime_inventory_retains_last_good_membership_across_scan_failure_and_republishes_repair()
{
    let boot = init_isolated_bootstrap("http://127.0.0.1", "{}\n");
    let pool = fixture_db_pool(&boot.database_url).await.unwrap();
    let owner = UserId::new(uuid::Uuid::new_v4().to_string());
    seed_user_row(&pool, &owner, &format!("{owner}@inventory.invalid"))
        .await
        .unwrap();
    let paths = PathsConfig {
        system: boot.system_path.display().to_string(),
        services: boot.services_path.display().to_string(),
        bin: boot.bin_path.display().to_string(),
        web_path: Some(boot.system_path.join("web").display().to_string()),
        storage: Some(boot.storage_path.display().to_string()),
        geoip_database: None,
    };
    let ctx = fixture_app_context_with(
        &pool,
        &boot.database_url,
        paths,
        std::sync::Arc::new(systemprompt_marketplace::AllowAllFilter),
    )
    .unwrap();
    skill(&boot.services_path, "# generation one\n");

    let first = inventory::refresh(&ctx, &owner).await.unwrap();
    assert_eq!(first.entries, 1);
    assert!(first.last_error.is_none());
    let captures = inventory::prepare_baselines(
        &ctx,
        &owner,
        &owner,
        &BaselinePreparation {
            operation_id: TaskId::generate(),
            after: None,
            limit: 100,
        },
    )
    .await
    .unwrap();
    assert_eq!(captures.len(), 1);
    assert_eq!(captures[0].status, "ready");
    let published = inventory::publish_latest(&ctx, &owner, &owner)
        .await
        .unwrap();
    assert_eq!(published.len(), 1);
    assert_eq!(published[0].status, LatestPublicationStatus::Published);
    assert_eq!(published[0].generation, Some(1));
    let last_good = ctx
        .managed_repository()
        .inventory_status(&owner)
        .await
        .unwrap();

    let skills = boot.services_path.join("skills");
    let retained_skills = boot.services_path.join("skills-retained");
    std::fs::rename(&skills, &retained_skills).unwrap();
    std::os::unix::fs::symlink(&retained_skills, &skills).unwrap();
    let error = inventory::refresh(&ctx, &owner)
        .await
        .expect_err("a symlinked configured catalog must fail the refresh");
    assert!(
        error
            .to_string()
            .contains("Configured catalog is a symlink")
    );
    let failed = ctx
        .managed_repository()
        .inventory_status(&owner)
        .await
        .unwrap();
    assert_eq!(failed.generation, last_good.generation);
    assert_eq!(failed.entries, 1, "last good membership remains visible");
    assert_eq!(
        failed.last_error.as_deref(),
        Some(
            "Inventory scan failed; previous membership retained, inspect configured catalog availability"
        )
    );

    std::fs::remove_file(&skills).unwrap();
    std::fs::rename(&retained_skills, &skills).unwrap();
    skill(&boot.services_path, "# generation two\n");
    let repaired = inventory::refresh(&ctx, &owner).await.unwrap();
    assert!(repaired.generation > first.generation);
    assert_eq!(repaired.entries, 1);
    assert!(repaired.last_error.is_none());
    let republished = inventory::publish_latest(&ctx, &owner, &owner)
        .await
        .unwrap();
    assert_eq!(republished.len(), 1);
    assert_eq!(republished[0].status, LatestPublicationStatus::Published);
    assert_eq!(republished[0].generation, Some(2));
}
