use systemprompt_identifiers::NotificationRuleId;
use systemprompt_models::notifications::{Attention, DeclError, RuleAction, RuleDecl, RuleMatch};
use systemprompt_notifications::NotificationsError;
use systemprompt_notifications::repository::NotificationRuleRepository;
use systemprompt_traits::RepositoryError;

use crate::harness::Harness;

fn rule(priority: i32, action: RuleAction) -> RuleDecl {
    RuleDecl {
        id: NotificationRuleId::generate(),
        name: format!("rule {priority}"),
        priority,
        r#match: RuleMatch {
            kinds: vec!["mentioned".to_owned()],
            ..RuleMatch::default()
        },
        action,
    }
}

fn mute() -> RuleAction {
    RuleAction {
        mute: true,
        ..RuleAction::default()
    }
}

#[tokio::test]
async fn own_rules_replace_the_whole_list_in_priority_order() {
    let h = Harness::new("notif_own_rules").await;
    let member = h.user("member", &["sales"]).await;
    assert!(h.service.own_rules(&member).await.expect("list").is_empty());

    let late = rule(5, mute());
    let early = rule(
        1,
        RuleAction {
            cap: Some(Attention::Agent),
            ..RuleAction::default()
        },
    );
    h.service
        .set_own_rules(&member, vec![late.clone(), early.clone()])
        .await
        .expect("set");
    let stored = h.service.own_rules(&member).await.expect("list");
    assert_eq!(stored, vec![early.clone(), late]);

    h.service
        .set_own_rules(&member, vec![early.clone()])
        .await
        .expect("replace");
    assert_eq!(
        h.service.own_rules(&member).await.expect("list"),
        vec![early]
    );

    let by_owner = NotificationRuleRepository::list_for_owners(
        h.pool.write_pool().as_ref(),
        std::slice::from_ref(&member),
    )
    .await
    .expect("by owner");
    assert_eq!(by_owner[&member].len(), 1);
    assert!(
        NotificationRuleRepository::list_global(h.pool.write_pool().as_ref())
            .await
            .expect("global")
            .is_empty(),
        "a user's rule is not a declared rule"
    );
    h.finish().await;
}

#[tokio::test]
async fn invalid_own_rules_are_refused_and_nothing_is_written() {
    let h = Harness::new("notif_own_rules_bad").await;
    let member = h.user("member", &["sales"]).await;
    let twice = rule(1, mute());
    let err = h
        .service
        .set_own_rules(&member, vec![twice.clone(), twice.clone()])
        .await
        .unwrap_err();
    assert!(
        matches!(err, NotificationsError::InvalidDeclaration(DeclError::DuplicateRule(ref id)) if *id == twice.id),
        "{err:?}"
    );
    let inverted = rule(
        1,
        RuleAction {
            cap: Some(Attention::Silent),
            floor: Some(Attention::Human),
            ..RuleAction::default()
        },
    );
    let err = h
        .service
        .set_own_rules(&member, vec![inverted])
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            NotificationsError::InvalidDeclaration(DeclError::RuleFloorAboveCap(_))
        ),
        "{err:?}"
    );
    assert!(h.service.own_rules(&member).await.expect("list").is_empty());
    h.finish().await;
}

#[tokio::test]
async fn an_id_held_by_a_declared_or_another_users_rule_is_refused() {
    let h = Harness::new("notif_own_rules_taken").await;
    let member = h.user("member", &["sales"]).await;
    let other = h.user("other", &["sales"]).await;
    let declared = h.declare_rule(RuleMatch::default(), mute()).await;

    let mut colliding = rule(1, mute());
    colliding.id = declared.clone();
    let err = h
        .service
        .set_own_rules(&member, vec![colliding.clone()])
        .await
        .unwrap_err();
    assert!(
        matches!(err, NotificationsError::DeclaredByConfig { ref key, .. } if *key == declared.to_string()),
        "{err:?}"
    );

    let theirs = rule(1, mute());
    h.service
        .set_own_rules(&other, vec![theirs.clone()])
        .await
        .expect("other's rule");
    let err = h
        .service
        .set_own_rules(&member, vec![theirs])
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            NotificationsError::Repository(RepositoryError::Conflict { .. })
        ),
        "{err:?}"
    );

    let err = NotificationRuleRepository::upsert_declared(
        h.pool.write_pool().as_ref(),
        &h.service.own_rules(&other).await.expect("list")[0],
        "code:rules.yaml",
    )
    .await
    .unwrap_err();
    assert!(matches!(err, RepositoryError::Conflict { .. }), "{err:?}");

    NotificationRuleRepository::delete_declared(h.pool.write_pool().as_ref(), &declared)
        .await
        .expect("delete");
    assert!(
        NotificationRuleRepository::list_global(h.pool.write_pool().as_ref())
            .await
            .expect("global")
            .is_empty()
    );
    assert!(h.service.own_rules(&member).await.expect("list").is_empty());
    h.finish().await;
}
