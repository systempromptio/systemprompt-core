//! Organization access retains publisher identity and personal fallback
//! semantics.

use crate::managed_resolution::{disk_catalog_with, fixture, publish, withdraw};
use systemprompt_marketplace::managed::{ManagedSkillResolution, OrganizationSkillResolver};
use systemprompt_traits::{ManagedSkillResolver, SkillResolution, WithheldReason};

#[tokio::test]
async fn organization_grants_preserve_exact_publication_and_revocation() {
    let f = fixture().await.expect("database fixture");
    let consumer = fixture().await.expect("consumer fixture");
    publish(&f).await;
    let resolver = OrganizationSkillResolver::new(f.repository.clone(), f.owner.clone());
    let runtime: &dyn ManagedSkillResolver = &resolver;
    assert_eq!(
        runtime
            .resolve_skill(&consumer.owner, &f.key)
            .await
            .expect("denied"),
        SkillResolution::Withheld(WithheldReason::NotGranted)
    );
    f.repository
        .set_consumer_grant(&f.owner, &f.resource, &consumer.owner, true)
        .await
        .expect("grant");
    let ManagedSkillResolution::Published(owner) = f
        .resolver
        .resolve_skill(&f.owner, &f.key)
        .await
        .expect("owner")
    else {
        panic!("publication");
    };
    let ManagedSkillResolution::Published(received) = resolver
        .resolve_skill(&consumer.owner, &f.key)
        .await
        .expect("consumer")
    else {
        panic!("grant resolves");
    };
    assert_eq!(received.publication_id, owner.publication_id);
    assert_eq!(received.resource_id, f.resource);
    assert_eq!(received.revision_id, f.revision);
    assert_eq!(received.generation, owner.generation);
    assert_eq!(received.bundle_digest, owner.bundle_digest);
    let (_dir, disk) = disk_catalog_with(&f.key);
    let granted = disk
        .clone()
        .with_organization_skills(f.repository.clone(), &f.owner, &consumer.owner)
        .await
        .expect("catalog");
    assert!(
        granted
            .as_content()
            .skills
            .iter()
            .any(|skill| skill.instructions.contains("managed instructions"))
    );
    f.repository
        .set_consumer_grant(&f.owner, &f.resource, &consumer.owner, false)
        .await
        .expect("revoke");
    f.repository
        .retain_consumer_catalog_grant(&f.owner, &f.resource, &consumer.owner)
        .await
        .expect("retention cannot regrant");
    assert_eq!(
        runtime
            .resolve_skill(&consumer.owner, &f.key)
            .await
            .expect("revoked"),
        SkillResolution::Withheld(WithheldReason::NotGranted)
    );
    let denied = disk
        .with_organization_skills(f.repository.clone(), &f.owner, &consumer.owner)
        .await
        .expect("catalog");
    assert!(denied.as_content().skills.is_empty());
    assert!(denied.as_content().managed_files.is_empty());
    f.repository
        .set_consumer_grant(&f.owner, &f.resource, &consumer.owner, true)
        .await
        .expect("grant");
    withdraw(&f).await;
    assert_eq!(
        runtime
            .resolve_skill(&consumer.owner, &f.key)
            .await
            .expect("withdrawn"),
        SkillResolution::Withheld(WithheldReason::Withdrawn)
    );
}

#[tokio::test]
async fn organization_unknown_key_preserves_personal_publication_and_withdrawal() {
    let organization = fixture().await.expect("organization fixture");
    let personal = fixture().await.expect("personal fixture");
    let resolver =
        OrganizationSkillResolver::new(organization.repository.clone(), organization.owner);
    publish(&personal).await;
    let ManagedSkillResolution::Published(received) = resolver
        .resolve_skill(&personal.owner, &personal.key)
        .await
        .expect("personal")
    else {
        panic!("personal publication must survive");
    };
    assert_eq!(received.resource_id, personal.resource);
    assert_eq!(received.revision_id, personal.revision);
    withdraw(&personal).await;
    assert!(
        matches!(resolver.resolve_skill(&personal.owner, &personal.key).await.expect("personal withdrawal"), ManagedSkillResolution::Withheld(reason) if *reason == WithheldReason::Withdrawn)
    );
    assert!(matches!(
        resolver
            .resolve_skill(&personal.owner, "unknown_key")
            .await
            .expect("unknown"),
        ManagedSkillResolution::NotManaged
    ));
}

#[tokio::test]
async fn anonymous_catalog_withholds_managed_keys_but_preserves_public_disk() {
    let f = fixture().await.expect("fixture");
    publish(&f).await;
    let (dir, _) = disk_catalog_with(&f.key);
    crate::helpers::write_skill_on_disk(dir.path(), "ordinary_public");
    let disk = systemprompt_marketplace::CatalogContent::load(
        &systemprompt_models::services::ServicesConfig::default(),
        dir.path(),
        "https://api.example.invalid",
    )
    .expect("disk");
    let anonymous = disk
        .without_organization_skills(f.repository, &f.owner)
        .await
        .expect("anonymous");
    assert_eq!(anonymous.as_content().skills.len(), 1);
    assert_eq!(
        anonymous.as_content().skills[0].id.as_str(),
        "ordinary_public"
    );
    assert!(anonymous.as_content().managed_files.is_empty());
}

#[tokio::test]
async fn organization_key_never_falls_through_to_personal_shadow() {
    let f = fixture().await.expect("organization");
    let personal = crate::managed_resolution::fixture_with_key(f.key.clone())
        .await
        .expect("personal same key");
    publish(&personal).await;
    let resolver = OrganizationSkillResolver::new(f.repository.clone(), f.owner.clone());
    assert!(
        matches!(resolver.resolve_skill(&personal.owner, &f.key).await.expect("not granted"), ManagedSkillResolution::Withheld(reason) if *reason == WithheldReason::NotGranted)
    );
    publish(&f).await;
    f.repository
        .set_consumer_grant(&f.owner, &f.resource, &personal.owner, true)
        .await
        .expect("grant");
    let ManagedSkillResolution::Published(received) = resolver
        .resolve_skill(&personal.owner, &f.key)
        .await
        .expect("organization")
    else {
        panic!("organization publication");
    };
    assert_eq!(received.resource_id, f.resource);
    assert_ne!(received.resource_id, personal.resource);
    withdraw(&f).await;
    assert!(
        matches!(resolver.resolve_skill(&personal.owner, &f.key).await.expect("withdrawal"), ManagedSkillResolution::Withheld(reason) if *reason == WithheldReason::Withdrawn)
    );
    let (_dir, disk) = disk_catalog_with(&f.key);
    let catalog = disk
        .with_organization_skills(f.repository, &f.owner, &personal.owner)
        .await
        .expect("catalog");
    assert!(catalog.as_content().skills.is_empty());
    assert!(catalog.as_content().managed_files.is_empty());
}

#[tokio::test]
async fn another_owner_sees_the_key_as_unmanaged() {
    let Some(f) = fixture().await else {
        return;
    };
    publish(&f).await;
    let stranger =
        systemprompt_identifiers::UserId::new(format!("managed-stranger-{}", uuid::Uuid::new_v4()));
    assert!(matches!(
        f.resolver
            .resolve_skill(&stranger, &f.key)
            .await
            .expect("resolve"),
        ManagedSkillResolution::NotManaged
    ));
}

#[tokio::test]
async fn generic_catalog_reoverlay_removes_withdrawn_revision_files() {
    let f = fixture().await.expect("fixture");
    publish(&f).await;
    let (_dir, disk) = disk_catalog_with(&f.key);
    let published = disk
        .with_managed_skills(f.repository.clone(), &f.owner)
        .await
        .expect("published catalog");
    assert_eq!(published.as_content().managed_files.len(), 1);
    withdraw(&f).await;
    let withdrawn = published
        .with_managed_skills(f.repository, &f.owner)
        .await
        .expect("withdrawn catalog");
    assert!(withdrawn.as_content().skills.is_empty());
    assert!(withdrawn.as_content().managed_files.is_empty());
}

#[tokio::test]
async fn catalog_includes_published_skill_without_grant() {
    let f = fixture().await.expect("database fixture");
    let consumer = fixture().await.expect("consumer fixture");
    publish(&f).await;
    let resolver = OrganizationSkillResolver::new(f.repository.clone(), f.owner.clone());
    let ManagedSkillResolution::Published(received) = resolver
        .resolve_skill_for_catalog(&consumer.owner, &f.key)
        .await
        .expect("catalog resolution")
    else {
        panic!("published skill reaches the catalogue without a grant");
    };
    assert_eq!(received.resource_id, f.resource);
    assert_eq!(received.revision_id, f.revision);
    let (_dir, disk) = disk_catalog_with(&f.key);
    let catalog = disk
        .with_organization_skills(f.repository.clone(), &f.owner, &consumer.owner)
        .await
        .expect("catalog");
    assert!(
        catalog
            .as_content()
            .skills
            .iter()
            .any(|skill| skill.instructions.contains("managed instructions"))
    );
    assert_eq!(catalog.as_content().managed_files.len(), 1);
    assert!(
        matches!(resolver.resolve_skill(&consumer.owner, &f.key).await.expect("runtime"), ManagedSkillResolution::Withheld(reason) if *reason == WithheldReason::NotGranted)
    );
    let runtime: &dyn ManagedSkillResolver = &resolver;
    assert_eq!(
        runtime
            .resolve_skill(&consumer.owner, &f.key)
            .await
            .expect("runtime trait"),
        SkillResolution::Withheld(WithheldReason::NotGranted)
    );
}

#[tokio::test]
async fn explicit_revocation_withholds_from_catalog() {
    let f = fixture().await.expect("database fixture");
    let consumer = fixture().await.expect("consumer fixture");
    publish(&f).await;
    let resolver = OrganizationSkillResolver::new(f.repository.clone(), f.owner.clone());
    f.repository
        .set_consumer_grant(&f.owner, &f.resource, &consumer.owner, false)
        .await
        .expect("revoke");
    assert!(
        matches!(resolver.resolve_skill_for_catalog(&consumer.owner, &f.key).await.expect("revoked"), ManagedSkillResolution::Withheld(reason) if *reason == WithheldReason::NotGranted)
    );
    let (_dir, disk) = disk_catalog_with(&f.key);
    let denied = disk
        .clone()
        .with_organization_skills(f.repository.clone(), &f.owner, &consumer.owner)
        .await
        .expect("catalog");
    assert!(denied.as_content().skills.is_empty());
    assert!(denied.as_content().managed_files.is_empty());
    f.repository
        .retain_consumer_catalog_grant(&f.owner, &f.resource, &consumer.owner)
        .await
        .expect("retention cannot regrant");
    assert!(
        matches!(resolver.resolve_skill_for_catalog(&consumer.owner, &f.key).await.expect("still revoked"), ManagedSkillResolution::Withheld(reason) if *reason == WithheldReason::NotGranted)
    );
    let still_denied = disk
        .with_organization_skills(f.repository.clone(), &f.owner, &consumer.owner)
        .await
        .expect("catalog");
    assert!(still_denied.as_content().skills.is_empty());
    let ManagedSkillResolution::Published(owner) = resolver
        .resolve_skill_for_catalog(&f.owner, &f.key)
        .await
        .expect("owner")
    else {
        panic!("owner is never withheld");
    };
    assert_eq!(owner.resource_id, f.resource);
}
