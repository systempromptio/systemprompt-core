//! Guards the schema↔enum contract for `governance_decisions.actor_kind`.
//!
//! Asserts that every `ActorKind::as_str()` value is named in both the base
//! schema CHECK and the most recent extension migration. Extending the enum
//! without extending the constraint fails this test instead of silently
//! rejecting rows at runtime.

use systemprompt_identifiers::{ActorKind, UserId};

const SCHEMA_SQL: &str =
    include_str!("../../../../../../infra/security/schema/governance_decisions.sql");
const MIGRATION_SQL: &str =
    include_str!("../../../../../../infra/security/schema/migrations/005_actor_kind_extend.sql");

fn all_variants() -> Vec<ActorKind> {
    let uid = UserId::new("u");
    vec![
        ActorKind::User,
        ActorKind::Anonymous,
        ActorKind::System,
        ActorKind::Job {
            job_name: "j".into(),
        },
        ActorKind::Mcp {
            server_name: "m".into(),
        },
        ActorKind::Agent {
            agent_id: "a".into(),
        },
    ]
    .into_iter()
    .inspect(|k| {
        let _ = k.actor_id(&uid);
    })
    .collect()
}

#[test]
fn schema_check_lists_every_actor_kind() {
    for kind in all_variants() {
        let name = kind.as_str();
        let token = format!("'{name}'");
        assert!(
            SCHEMA_SQL.contains(&token),
            "governance_decisions.sql CHECK is missing ActorKind::{name} ({token}); the enum \
             grew without a matching schema update — see 005_actor_kind_extend.sql"
        );
    }
}

#[test]
fn migration_lists_every_actor_kind() {
    for kind in all_variants() {
        let name = kind.as_str();
        let token = format!("'{name}'");
        assert!(
            MIGRATION_SQL.contains(&token),
            "005_actor_kind_extend.sql is missing ActorKind::{name} ({token})"
        );
    }
}
