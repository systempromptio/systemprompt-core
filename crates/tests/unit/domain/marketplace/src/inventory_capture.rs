use super::*;

#[tokio::test]
async fn failed_complete_scan_preserves_membership_and_records_health() {
    let f = Fixture::new().await;
    f.skill("retained", true);
    f.refresh().await;
    let initial = f
        .repository
        .inventory_status(&f.owner)
        .await
        .expect("status");
    let result = InventoryService::new(f.repository.clone())
        .refresh(
            &f.owner,
            &f.root.path().join("missing"),
            &ServicesConfig::default(),
        )
        .await;
    assert!(result.is_err());
    let status = f
        .repository
        .inventory_status(&f.owner)
        .await
        .expect("status");
    assert_eq!(status.generation, initial.generation);
    assert!(status.last_error.is_some());
    assert_eq!(
        f.repository
            .inventory(&f.owner, None, 100)
            .await
            .expect("retained")
            .len(),
        1
    );
}

#[tokio::test]
async fn bounded_pages_and_immutable_bindings_do_not_adopt_same_named_resources() {
    let f = Fixture::new().await;
    f.skill("local", true);
    let (first, _) = f.imported("first").await;
    let (second, _) = f.imported("second").await;
    f.refresh().await;
    let page = f
        .repository
        .inventory(&f.owner, None, 1)
        .await
        .expect("first page");
    let next = f
        .repository
        .inventory(&f.owner, Some(&page[0].entry_id), 1)
        .await
        .expect("second page");
    assert_ne!(page[0].entry_id, next[0].entry_id);
    let id = configured_identity(&f.owner, "skill", "local");
    f.repository
        .bind_inventory_resource(&f.owner, &f.owner, &id, &first)
        .await
        .expect("bind");
    f.repository
        .bind_inventory_resource(&f.owner, &f.owner, &id, &first)
        .await
        .expect("identical retry");
    assert!(
        f.repository
            .bind_inventory_resource(&f.owner, &f.owner, &id, &second)
            .await
            .is_err()
    );
}
