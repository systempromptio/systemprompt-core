//! Baseline capture refuses a configured source bound to a different services
//! root, and accepts one whose binding resolves to the active root.

use super::*;

impl Fixture {
    async fn publish_from(&self, root: &std::path::Path) -> Vec<LatestPublication> {
        InventoryService::new(self.repository.clone())
            .publish_latest(
                &BaselineScope {
                    owner: &self.owner,
                    actor: &self.owner,
                    root,
                    services: &ServicesConfig::default(),
                },
                &mut PublishGuard::default(),
            )
            .await
            .expect("publish latest")
    }
}

fn copy_skill(from: &std::path::Path, to: &std::path::Path, key: &str, body: &str) {
    let target = to.join("skills").join(key);
    std::fs::create_dir_all(&target).expect("directory");
    std::fs::copy(
        from.join("skills").join(key).join("config.yaml"),
        target.join("config.yaml"),
    )
    .expect("config");
    std::fs::write(target.join("SKILL.md"), body).expect("skill");
}

#[tokio::test]
async fn a_source_bound_to_another_root_blocks_capture() {
    let f = Fixture::new().await;
    f.skill("rooted", true);
    f.refresh().await;
    let first = f.publish().await.remove(0);
    assert_eq!(first.status, LatestPublicationStatus::Published);
    let other = tempfile::tempdir().expect("other root");
    copy_skill(f.root.path(), other.path(), "rooted", "# moved\n");

    let outcome = f.publish_from(other.path()).await.remove(0);

    assert_eq!(outcome.status, LatestPublicationStatus::Blocked);
    assert!(
        outcome
            .diagnostic
            .as_deref()
            .is_some_and(|text| text.contains("does not match the active services root")),
        "{:?}",
        outcome.diagnostic
    );
    assert_eq!(outcome.revision_id, first.revision_id);
}

#[cfg(unix)]
#[tokio::test]
async fn a_binding_that_resolves_to_the_active_root_is_accepted() {
    let f = Fixture::new().await;
    f.skill("aliased", true);
    let links = tempfile::tempdir().expect("link root");
    let alias = links.path().join("services");
    std::os::unix::fs::symlink(f.root.path(), &alias).expect("alias");
    InventoryService::new(f.repository.clone())
        .refresh(&f.owner, &alias, &ServicesConfig::default())
        .await
        .expect("refresh through alias");
    let first = f.publish_from(&alias).await.remove(0);
    assert_eq!(first.status, LatestPublicationStatus::Published);
    std::fs::write(f.root.path().join("skills/aliased/SKILL.md"), "# edited\n").expect("edit");

    let outcome = f.publish().await.remove(0);

    assert_eq!(
        outcome.status,
        LatestPublicationStatus::Published,
        "{:?}",
        outcome.diagnostic
    );
    assert_eq!(outcome.generation, Some(2));
}
