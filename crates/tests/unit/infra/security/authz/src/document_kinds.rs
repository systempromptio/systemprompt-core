//! `document` and `folder` as access-control entity kinds: the tags round-trip,
//! their ids must be uuids, they have no parent chain, a closed declaration
//! admits only a matching role band, and the catalog CHECK accepts every kind.

use std::str::FromStr;

use systemprompt_governance::authz::resolver::{ResolveInput, resolve};
use systemprompt_governance::authz::rules_file::EntityTarget;
use systemprompt_governance::authz::types::{
    Access, AccessRule, Decision, EntityKind, EntityRef, RuleType,
};
use systemprompt_governance::authz::{AccessControlRepository, UpsertRuleParams};
use systemprompt_identifiers::{DocumentFolderId, RuleId};
use systemprompt_test_fixtures::{fixture_user_id, test_db_pool};

const FOLDER: &str = "6f1c2a4e-8b3d-4e5f-9a7b-1c2d3e4f5a6b";

#[test]
fn document_and_folder_tags_round_trip() {
    for (kind, tag) in [
        (EntityKind::Document, "document"),
        (EntityKind::Folder, "folder"),
    ] {
        assert_eq!(kind.as_str(), tag);
        assert_eq!(kind.to_string(), tag);
        assert_eq!(EntityKind::from_str(tag).unwrap(), kind);
        assert!(EntityKind::ALL.contains(&kind));
    }
}

#[test]
fn every_kind_in_all_round_trips_through_its_tag() {
    for kind in EntityKind::ALL {
        assert_eq!(EntityKind::from_str(kind.as_str()).unwrap(), *kind);
    }
}

#[test]
fn a_document_ref_takes_a_uuid_id() {
    let entity = EntityRef::from_kind_and_id(EntityKind::Document, FOLDER).unwrap();
    assert_eq!(entity.kind(), EntityKind::Document);
    assert_eq!(entity.id_str(), FOLDER);
}

#[test]
fn a_non_uuid_document_or_folder_id_is_rejected() {
    assert!(EntityRef::from_kind_and_id(EntityKind::Document, "not-a-uuid").is_err());
    assert!(EntityRef::from_kind_and_id(EntityKind::Folder, "not-a-uuid").is_err());
}

#[test]
fn a_folder_ref_displays_as_kind_and_uuid() {
    let entity = EntityRef::Folder(DocumentFolderId::try_new(FOLDER).unwrap());
    assert_eq!(entity.to_string(), format!("folder:{FOLDER}"));
}

#[test]
fn a_rules_file_entity_names_a_folder() {
    let target = EntityTarget::from_str(&format!("folder/{FOLDER}")).unwrap();
    assert_eq!(target.kind, EntityKind::Folder);
    assert_eq!(target.id, FOLDER);
}

fn closed_folder_decision(user_roles: &[String]) -> Decision {
    let entity = EntityRef::Folder(DocumentFolderId::try_new(FOLDER).unwrap());
    let rules = [AccessRule {
        id: RuleId::new("folder-role-editors"),
        rule_type: RuleType::ROLE,
        rule_value: "editors".to_owned(),
        access: Access::Allow,
        justification: None,
    }];
    let user = fixture_user_id();
    resolve(ResolveInput {
        entity: &entity,
        rules: &rules,
        user_id: &user,
        user_roles,
        default_included: Some(false),
        parents: &[],
        attributes: &systemprompt_governance::authz::NO_SUBJECT_ATTRIBUTES,
        dimensions: &[],
    })
}

#[test]
fn a_closed_folder_denies_a_user_outside_the_role_band() {
    let decision = closed_folder_decision(&["user".to_owned()]);
    assert!(!decision.permits(), "got: {decision:?}");
}

#[test]
fn a_closed_folder_allows_a_user_in_the_role_band() {
    let decision = closed_folder_decision(&["editors".to_owned()]);
    assert!(decision.permits(), "got: {decision:?}");
}

#[tokio::test]
async fn the_catalog_accepts_a_folder_rule() {
    let pool = test_db_pool().await;
    let repo = AccessControlRepository::new(&pool);
    let folder = uuid::Uuid::new_v4().to_string();
    repo.upsert_entity(EntityKind::Folder, &folder, false, "migration")
        .await
        .expect("folder catalog row");
    repo.upsert_rule(UpsertRuleParams {
        entity_type: EntityKind::Folder,
        entity_id: &folder,
        rule_type: RuleType::ROLE,
        rule_value: "editors",
        access: Access::Allow,
        justification: None,
        source: "migration",
    })
    .await
    .expect("folder rule");

    let row = repo
        .get_entity(EntityKind::Folder, &folder)
        .await
        .expect("lookup")
        .expect("folder row");
    assert_eq!(row.kind, EntityKind::Folder);
}

#[tokio::test]
async fn the_catalog_accepts_every_entity_kind() {
    let pool = test_db_pool().await;
    let repo = AccessControlRepository::new(&pool);
    for kind in EntityKind::ALL {
        let id = uuid::Uuid::new_v4().to_string();
        repo.upsert_entity(*kind, &id, false, "migration")
            .await
            .unwrap_or_else(|e| panic!("{kind} catalog row rejected: {e}"));
    }
}
