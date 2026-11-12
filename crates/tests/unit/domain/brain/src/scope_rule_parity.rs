//! DB-backed: the Rust visibility verdict and the SQL working-set keys of
//! `brain_fact_scope_keys` agree on which principals a fact's scope and
//! topics reach, for every scope level against every topic audience.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_brain::recall::held::HeldKeys;
use systemprompt_brain::recall::visible::{Verdict, Visibility};
use systemprompt_models::brain::{FactScope, Granularity, ScopeLevel};
use systemprompt_models::notifications::AudienceSubject;
use systemprompt_test_fixtures::test_db_pool;

use crate::recall_fakes::FakeAccess;
use crate::recall_fixtures::{brain_fact, principal, uid, with_scope, with_topic};
use crate::recall_visible::{READABLE, topic_decl};

struct Who {
    user: &'static str,
    project: Option<&'static str>,
    extra: &'static [&'static str],
}

const PRINCIPALS: &[Who] = &[
    Who {
        user: "bob",
        project: None,
        extra: &[],
    },
    Who {
        user: "carol",
        project: None,
        extra: &[],
    },
    Who {
        user: "dave",
        project: None,
        extra: &["role:lead"],
    },
    Who {
        user: "erin",
        project: None,
        extra: &["group:emea", "group:platform"],
    },
    Who {
        user: "fred",
        project: Some("apollo"),
        extra: &[],
    },
];

fn held(who: &Who) -> HeldKeys {
    let mut keys = vec![
        ScopeLevel::Org.as_str().to_owned(),
        format!("user:{}", who.user),
    ];
    if let Some(project) = who.project {
        keys.push(format!("project:{project}"));
    }
    keys.extend(who.extra.iter().map(|k| (*k).to_owned()));
    keys.sort();
    keys.dedup();
    HeldKeys {
        keys,
        enumerated: Vec::new(),
    }
}

fn topics(tag: &str) -> Vec<(String, Vec<AudienceSubject>)> {
    vec![
        (format!("open-{tag}"), vec![AudienceSubject::Everyone]),
        (
            format!("leads-{tag}"),
            vec![AudienceSubject::Role("lead".to_owned())],
        ),
        (
            format!("emea-{tag}"),
            vec![AudienceSubject::Dimension {
                kind: "group".to_owned(),
                value: "emea".to_owned(),
            }],
        ),
    ]
}

#[tokio::test]
async fn rust_verdicts_and_sql_scope_keys_reach_the_same_principals() {
    let pool = test_db_pool().await;
    let mut tx = pool.begin().await.expect("tx");
    let tag = uuid::Uuid::new_v4().simple().to_string()[..8].to_owned();
    let declared = topics(&tag);
    for (id, audience) in &declared {
        let audience: Vec<String> = audience.iter().map(ToString::to_string).collect();
        sqlx::query("INSERT INTO brain_topics (id, body, source) VALUES ($1, $2, 'test')")
            .bind(id)
            .bind(serde_json::json!({ "audience": audience }))
            .execute(&mut *tx)
            .await
            .expect("topic");
    }
    let decls = declared
        .iter()
        .map(|(id, audience)| topic_decl(id, audience.clone(), Granularity::Individual))
        .collect::<Vec<_>>();
    let scopes = [
        FactScope::User(uid("bob")),
        FactScope::Project("apollo".to_owned()),
        FactScope::Group("platform".to_owned()),
        FactScope::Org,
    ];
    let mut topic_sets: Vec<Vec<String>> = vec![Vec::new()];
    topic_sets.extend(declared.iter().map(|(id, _)| vec![id.clone()]));

    let mut cases = 0;
    for scope in &scopes {
        for set in &topic_sets {
            let (level, key) = match scope {
                FactScope::User(owner) => ("user", owner.as_str().to_owned()),
                FactScope::Project(p) => ("project", p.clone()),
                FactScope::Group(g) => ("group", g.clone()),
                FactScope::Org => ("org", String::new()),
            };
            let sql_keys: Vec<String> =
                sqlx::query_scalar("SELECT k FROM brain_fact_scope_keys($1, $2, $3) k")
                    .bind(level)
                    .bind(&key)
                    .bind(set)
                    .fetch_all(&mut *tx)
                    .await
                    .expect("scope keys");
            let mut fact = with_scope(brain_fact(1, "a"), scope.clone());
            for topic in set {
                fact = with_topic(fact, topic);
            }
            for who in PRINCIPALS {
                let held = held(who);
                let sql_reaches = sql_keys.iter().any(|k| held.keys.contains(k));
                let visibility =
                    Visibility::new(Arc::new(FakeAccess(READABLE)), Duration::from_millis(20))
                        .with_topics(decls.clone())
                        .with_held(held);
                let rust_reaches = visibility
                    .check(&fact, &principal(who.user, who.project))
                    .await
                    == Verdict::Visible;
                assert_eq!(
                    rust_reaches, sql_reaches,
                    "scope {scope:?} topics {set:?} principal {} sql keys {sql_keys:?}",
                    who.user
                );
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 4 * 4 * PRINCIPALS.len());
    tx.rollback().await.expect("rollback");
}
