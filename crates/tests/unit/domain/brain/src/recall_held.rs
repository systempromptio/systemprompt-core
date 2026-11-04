use std::sync::Arc;

use systemprompt_brain::recall::held::{build, key_for, project_key, user_key};
use systemprompt_extension::notifications::NotificationAudienceResolver;
use systemprompt_models::notifications::AudienceSubject;

use crate::recall_fakes::{EnumeratingResolver, FakeResolver, SlowResolver};
use crate::recall_fixtures::uid;

#[test]
fn keys_render_as_the_subject_display_form_with_everyone_folded_into_org() {
    assert_eq!(key_for(&AudienceSubject::Everyone), "org");
    assert_eq!(
        key_for(&AudienceSubject::Role("lead".to_owned())),
        "role:lead"
    );
    assert_eq!(key_for(&AudienceSubject::User(uid("bob"))), "user:bob");
    assert_eq!(
        key_for(&AudienceSubject::Dimension {
            kind: "group".to_owned(),
            value: "platform".to_owned()
        }),
        "group:platform"
    );
    assert_eq!(user_key(&uid("bob")), "user:bob");
    assert_eq!(project_key("apollo"), "project:apollo");
}

#[tokio::test]
async fn held_keys_are_org_user_project_and_every_enumerated_subject_sorted_once() {
    let roles = Arc::new(EnumeratingResolver::new(
        "roles",
        Some(vec![
            AudienceSubject::Role("lead".to_owned()),
            AudienceSubject::Role("admin".to_owned()),
            AudienceSubject::Everyone,
        ]),
    ));
    let groups = Arc::new(EnumeratingResolver::new(
        "groups",
        Some(vec![AudienceSubject::Dimension {
            kind: "group".to_owned(),
            value: "platform".to_owned(),
        }]),
    ));
    let silent: Arc<dyn NotificationAudienceResolver> =
        Arc::new(FakeResolver { member: uid("bob") });
    let held = build(
        &uid("bob"),
        Some("apollo"),
        &[Arc::clone(&roles) as _, Arc::clone(&groups) as _, silent],
    )
    .await;
    assert_eq!(
        held.keys,
        vec![
            "group:platform",
            "org",
            "project:apollo",
            "role:admin",
            "role:lead",
            "user:bob",
        ]
    );
    assert_eq!(held.enumerated, vec!["roles", "groups"]);
    assert!(held.holds(&AudienceSubject::Role("lead".to_owned())));
    assert!(!held.holds(&AudienceSubject::Role("intern".to_owned())));
    assert!(held.enumerated_by("roles"));
    assert!(
        !held.enumerated_by("fake"),
        "a resolver answering None is not enumerated"
    );
}

#[tokio::test]
async fn resolvers_are_asked_together_and_enumerated_in_resolver_order() {
    let slow = Arc::new(SlowResolver {
        inner: EnumeratingResolver::new("slow", Some(vec![AudienceSubject::Role("a".to_owned())])),
        delay_ms: 30,
    });
    let fast = Arc::new(EnumeratingResolver::new(
        "fast",
        Some(vec![AudienceSubject::Role("b".to_owned())]),
    ));
    let started = std::time::Instant::now();
    let held = build(
        &uid("bob"),
        None,
        &[Arc::clone(&slow) as _, Arc::clone(&fast) as _],
    )
    .await;
    assert_eq!(held.enumerated, vec!["slow", "fast"]);
    assert_eq!(held.keys, vec!["org", "role:a", "role:b", "user:bob"]);
    assert!(
        started.elapsed() < std::time::Duration::from_millis(55),
        "the two resolvers were not awaited one after the other"
    );
}

#[tokio::test]
async fn without_a_project_or_resolvers_the_principal_holds_org_and_itself() {
    let held = build(&uid("carol"), None, &[]).await;
    assert_eq!(held.keys, vec!["org", "user:carol"]);
    assert!(held.enumerated.is_empty());
}
