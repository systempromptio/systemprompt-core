use crate::consumer_fixture::fixture;
use systemprompt_models::feedback::receipts::ReadbackStatus;

#[tokio::test]
async fn acknowledged_installation_coverage_is_independent_of_unknown_usage() {
    let f = fixture().await;
    f.repo
        .refresh_installation_coverage(&f.owner)
        .await
        .unwrap();
    let initial = f
        .repo
        .installation_coverage(&f.owner, &f.request.resource_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(initial.eligible_devices, 1);
    assert_eq!(initial.current_acknowledged_devices, 0);
    f.repo
        .record_consumer_receipt(&f.credential.credential, &f.request)
        .await
        .unwrap();
    let first = f
        .repo
        .refresh_installation_coverage(&f.owner)
        .await
        .unwrap();
    let current = f
        .repo
        .installation_coverage(&f.owner, &f.request.resource_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.current_acknowledged_devices, 1);
    assert_eq!(current.current_verified_devices, 1);
    assert_eq!(current.acknowledged_installations, 1);
    let same = f
        .repo
        .refresh_installation_coverage(&f.owner)
        .await
        .unwrap();
    assert_eq!(first.generation, same.generation);
    f.repo
        .set_consumer_grant(&f.owner, &f.request.resource_id, &f.consumer, false)
        .await
        .unwrap();
    f.repo
        .refresh_installation_coverage(&f.owner)
        .await
        .unwrap();
    let revoked = f
        .repo
        .installation_coverage(&f.owner, &f.request.resource_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(revoked.eligible_devices, 0);
    assert_eq!(revoked.current_acknowledged_devices, 0);
    assert_eq!(revoked.acknowledged_installations, 1);
}
#[tokio::test]
async fn unavailable_readback_is_acknowledged_without_claiming_verification() {
    let f = fixture().await;
    let mut request = f.request.clone();
    for file in &mut request.files {
        file.mode_check = ReadbackStatus::Unavailable;
    }
    request.runtime_files.clear();
    f.repo
        .record_consumer_receipt(&f.credential.credential, &request)
        .await
        .unwrap();
    f.repo
        .refresh_installation_coverage(&f.owner)
        .await
        .unwrap();
    let coverage = f
        .repo
        .installation_coverage(&f.owner, &request.resource_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(coverage.current_acknowledged_devices, 1);
    assert_eq!(coverage.current_verified_devices, 0);
    assert_eq!(coverage.unverifiable_installations, 1);
}
