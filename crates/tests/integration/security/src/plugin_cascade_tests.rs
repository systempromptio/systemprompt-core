//! DB-backed coverage for the plugin level of the parent cascade: a plugin's
//! access rules govern every ruleless skill it ships, ahead of the marketplace.
//!
//! Each test scopes itself to unique ids so concurrent runs against the shared
//! `DATABASE_URL` never collide, and cleans up its rows.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{MarketplaceId, PluginId, SkillId};
use systemprompt_security::authz::{
    AccessControlRepository, BulkKeepQuery, ChainSources, EntityKind, MarketplaceSource,
    NO_SUBJECT_ATTRIBUTES, ParentChainIndex, allowed_ids,
};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool, fixture_user_id};
use uuid::Uuid;

struct Fixture {
    db: DbPool,
    pg: Arc<PgPool>,
    market: String,
    admin_plugin: String,
    user_plugin: String,
    admin_skill: String,
    user_skill: String,
    shared_skill: String,
}

impl Fixture {
    fn rows(&self) -> Vec<(&'static str, &str)> {
        vec![
            ("marketplace", &self.market),
            ("plugin", &self.admin_plugin),
            ("plugin", &self.user_plugin),
            ("skill", &self.admin_skill),
            ("skill", &self.user_skill),
            ("skill", &self.shared_skill),
        ]
    }

    fn sources(&self) -> ChainSources {
        let owners = |ids: &[&str]| {
            ids.iter()
                .map(|s| PluginId::new(*s))
                .collect::<BTreeSet<_>>()
        };
        ChainSources {
            marketplace: Some(MarketplaceSource {
                id: MarketplaceId::new(&self.market),
                fallback_default_included: Some(true),
            }),
            plugins: owners(&[&self.admin_plugin, &self.user_plugin]),
            skill_owners: BTreeMap::from([
                (
                    SkillId::new(&self.admin_skill),
                    owners(&[&self.admin_plugin]),
                ),
                (SkillId::new(&self.user_skill), owners(&[&self.user_plugin])),
                (
                    SkillId::new(&self.shared_skill),
                    owners(&[&self.admin_plugin, &self.user_plugin]),
                ),
            ]),
            marketplace_members: BTreeMap::new(),
        }
    }
}

async fn setup() -> Fixture {
    let url = fixture_database_url().expect("DATABASE_URL");
    let db = fixture_db_pool(&url).await.expect("connect test database");
    let pg = db.pool_arc().expect("read pool");
    let tag = Uuid::new_v4();
    let fixture = Fixture {
        db,
        pg,
        market: format!("mp-cascade-{tag}"),
        admin_plugin: format!("admin-plugin-{tag}"),
        user_plugin: format!("user-plugin-{tag}"),
        admin_skill: format!("admin_skill_{}", tag.simple()),
        user_skill: format!("user_skill_{}", tag.simple()),
        shared_skill: format!("shared_skill_{}", tag.simple()),
    };
    cleanup(&fixture).await;
    fixture
}

async fn cleanup(fixture: &Fixture) {
    for (kind, id) in fixture.rows() {
        sqlx::query("DELETE FROM access_control_rules WHERE entity_type=$1 AND entity_id=$2")
            .bind(kind)
            .bind(id)
            .execute(&*fixture.pg)
            .await
            .expect("cleanup rules");
        sqlx::query("DELETE FROM access_control_entities WHERE entity_type=$1 AND entity_id=$2")
            .bind(kind)
            .bind(id)
            .execute(&*fixture.pg)
            .await
            .expect("cleanup entities");
    }
}

async fn grant(fixture: &Fixture, kind: &str, id: &str, role: &str, default_included: bool) {
    sqlx::query(
        "INSERT INTO access_control_entities (entity_type, entity_id, default_included, source) \
         VALUES ($1, $2, $3, 'test')",
    )
    .bind(kind)
    .bind(id)
    .bind(default_included)
    .execute(&*fixture.pg)
    .await
    .expect("insert entity");
    sqlx::query(
        "INSERT INTO access_control_rules (id, entity_type, entity_id, rule_type, rule_value, \
         access) VALUES ($1, $2, $3, 'role', $4, 'allow')",
    )
    .bind(format!("rule-{}", Uuid::new_v4()))
    .bind(kind)
    .bind(id)
    .bind(role)
    .execute(&*fixture.pg)
    .await
    .expect("insert rule");
}

async fn visible_skills(fixture: &Fixture, roles: &[&str]) -> HashSet<String> {
    let repo = AccessControlRepository::new(&fixture.db).expect("repo");
    let index = ParentChainIndex::load(&repo, std::sync::Arc::new(fixture.sources()))
        .await
        .expect("load chain index");
    let roles: Vec<String> = roles.iter().map(|r| (*r).to_owned()).collect();
    let ids = vec![
        fixture.admin_skill.clone(),
        fixture.user_skill.clone(),
        fixture.shared_skill.clone(),
    ];
    allowed_ids(
        &repo,
        BulkKeepQuery {
            user_id: &fixture_user_id(),
            roles: &roles,
            kind: EntityKind::Skill,
            ids: &ids,
            chains: &index,
            attributes: &NO_SUBJECT_ATTRIBUTES,
            dimensions: &[],
        },
    )
    .await
    .expect("allowed_ids")
}

#[tokio::test]
async fn a_ruleless_skill_in_an_admin_gated_plugin_is_hidden_from_a_user() {
    let fixture = setup().await;
    grant(&fixture, "marketplace", &fixture.market, "user", true).await;
    grant(&fixture, "plugin", &fixture.admin_plugin, "admin", false).await;

    let user = visible_skills(&fixture, &["user"]).await;
    let admin = visible_skills(&fixture, &["user", "admin"]).await;

    assert!(
        !user.contains(&fixture.admin_skill),
        "the marketplace admits users, but the admin plugin closes its ruleless skill: {user:?}"
    );
    assert!(
        user.contains(&fixture.user_skill),
        "a ruleless plugin is transparent, so the marketplace grant reaches its skill: {user:?}"
    );
    assert!(
        user.contains(&fixture.shared_skill),
        "a skill owned by an admitting plugin as well is visible: {user:?}"
    );
    assert!(admin.contains(&fixture.admin_skill));
    assert!(admin.contains(&fixture.user_skill));

    cleanup(&fixture).await;
}
