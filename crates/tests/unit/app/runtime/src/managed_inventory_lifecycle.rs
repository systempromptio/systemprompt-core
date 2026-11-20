use std::sync::Arc;

use systemprompt_identifiers::{Actor, UserId};
use systemprompt_marketplace::inventory::{
    InventoryRefreshJob, InventoryRefresher, LatestPublicationStatus,
};
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{init_isolated_bootstrap, test_app_context_on, test_db_pool};
use systemprompt_traits::{Job, JobContext};

fn refresher(ctx: &AppContext) -> Arc<InventoryRefresher> {
    Arc::clone(
        ctx.kernel()
            .state()
            .get::<Arc<InventoryRefresher>>()
            .expect("the managed_resources member provides the inventory refresher"),
    )
}

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
    let pool = test_db_pool().await;
    systemprompt_test_fixtures::seed_fixture_system_admin(&pool)
        .await
        .expect("seed the system admin");
    let ctx = test_app_context_on(&boot).await;
    let inventory = refresher(&ctx);
    let owner = inventory.owner().clone();
    skill(&boot.services_path, "# generation one\n");

    let first = inventory.refresh().await.unwrap();
    assert_eq!(first.entries, 1);
    assert!(first.last_error.is_none());
    let published = inventory.publish_latest(&owner).await.unwrap();
    assert_eq!(published.len(), 1);
    assert_eq!(published[0].status, LatestPublicationStatus::Published);
    assert_eq!(published[0].generation, Some(1));
    let last_good = systemprompt_test_fixtures::fixture_managed_repository(&ctx)
        .inventory_status(&owner)
        .await
        .unwrap();

    let skills = boot.services_path.join("skills");
    let retained_skills = boot.services_path.join("skills-retained");
    std::fs::rename(&skills, &retained_skills).unwrap();
    std::os::unix::fs::symlink(&retained_skills, &skills).unwrap();
    let error = inventory
        .refresh()
        .await
        .expect_err("a symlinked configured catalog must fail the refresh");
    assert!(
        error
            .to_string()
            .contains("Configured catalog is a symlink")
    );
    let failed = systemprompt_test_fixtures::fixture_managed_repository(&ctx)
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
    let repaired = inventory.refresh().await.unwrap();
    assert!(repaired.generation > first.generation);
    assert_eq!(repaired.entries, 1);
    assert!(repaired.last_error.is_none());
    let republished = inventory.publish_latest(&owner).await.unwrap();
    assert_eq!(republished.len(), 1);
    assert_eq!(republished[0].status, LatestPublicationStatus::Published);
    assert_eq!(republished[0].generation, Some(2));
}

#[tokio::test]
async fn the_inventory_refresh_job_runs_twice_from_its_member_state() {
    let boot = init_isolated_bootstrap("http://127.0.0.1", "{}\n");
    let pool = test_db_pool().await;
    systemprompt_test_fixtures::seed_fixture_system_admin(&pool)
        .await
        .expect("seed the system admin");
    let ctx = test_app_context_on(&boot).await;
    let context = JobContext::new(
        Actor::job(UserId::new("inventory-job-admin"), "test".to_owned()),
        ctx.kernel().runtime_state().with(Arc::clone(&pool)),
    );
    let job = InventoryRefreshJob::default();
    assert_eq!(job.name(), "managed_inventory_refresh");
    assert!(job.execute(&context).await.expect("first refresh").success);
    assert!(job.execute(&context).await.expect("second refresh").success);
}
