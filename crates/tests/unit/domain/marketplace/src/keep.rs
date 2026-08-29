//! `keep_sets` — resolving which candidate entries a subject may see.
//!
//! The cascade is what these are about. A rule on a plugin governs every
//! ruleless skill that plugin ships, so one plugin rule covers the whole
//! bundle; the nearest level that declares a rule decides. `keep_sets` sits a
//! layer above `allowed_ids` and its job is to assemble the parent chain from
//! the candidate — in particular `skill_owners`, without which a skill loses
//! its plugin inheritance and falls back to the marketplace.
//!
//! Each test namespaces its ids with a uuid so concurrent runs against the
//! shared database cannot collide, and deletes its rows afterwards.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{MarketplaceId, UserId};
use systemprompt_marketplace::{KeepSetsSubject, MarketplaceCandidate, keep_sets};
use systemprompt_models::bridge::ids::{PluginId, SkillId};
use systemprompt_models::bridge::manifest::{PluginEntry, SkillEntry};
use systemprompt_security::authz::{AccessControlRepository, NO_SUBJECT_ATTRIBUTES};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_db_pool};
use uuid::Uuid;

struct Fixture {
    db: DbPool,
    pg: Arc<PgPool>,
    market: String,
    plugin: String,
    owned_skill: String,
    orphan_skill: String,
}

async fn setup() -> Fixture {
    let b = ensure_test_bootstrap();
    let db = fixture_db_pool(&b.database_url)
        .await
        .expect("the keep_sets tests need a reachable test database");
    let pg = db.pool_arc().expect("read pool");
    let tag = Uuid::new_v4().simple().to_string();
    let fixture = Fixture {
        db,
        pg,
        market: format!("mp-keep-{tag}"),
        plugin: format!("keep-plugin-{tag}"),
        owned_skill: format!("owned_skill_{tag}"),
        orphan_skill: format!("orphan_skill_{tag}"),
    };
    cleanup(&fixture).await;
    fixture
}

fn rows(f: &Fixture) -> Vec<(&'static str, &str)> {
    vec![
        ("marketplace", &f.market),
        ("plugin", &f.plugin),
        ("skill", &f.owned_skill),
        ("skill", &f.orphan_skill),
    ]
}

async fn cleanup(f: &Fixture) {
    for (kind, id) in rows(f) {
        sqlx::query("DELETE FROM access_control_rules WHERE entity_type=$1 AND entity_id=$2")
            .bind(kind)
            .bind(id)
            .execute(&*f.pg)
            .await
            .expect("cleanup rules");
        sqlx::query("DELETE FROM access_control_entities WHERE entity_type=$1 AND entity_id=$2")
            .bind(kind)
            .bind(id)
            .execute(&*f.pg)
            .await
            .expect("cleanup entities");
    }
}

async fn grant_role(f: &Fixture, kind: &str, id: &str, role: &str) {
    sqlx::query(
        "INSERT INTO access_control_entities (entity_type, entity_id, default_included, source) \
         VALUES ($1, $2, false, 'test')",
    )
    .bind(kind)
    .bind(id)
    .execute(&*f.pg)
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
    .execute(&*f.pg)
    .await
    .expect("insert rule");
}

const ZERO_DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn skill(id: &str) -> SkillEntry {
    use systemprompt_models::bridge::ids::{Sha256Digest, SkillName};
    SkillEntry {
        id: SkillId::try_new(id).expect("valid skill id"),
        name: SkillName::try_new(id).expect("valid skill name"),
        description: String::new(),
        file_path: String::new(),
        tags: vec![],
        sha256: Sha256Digest::try_new(ZERO_DIGEST).expect("valid digest"),
        instructions: String::new(),
        hosts: Vec::new(),
    }
}

fn plugin(id: &str) -> PluginEntry {
    use systemprompt_models::bridge::ids::Sha256Digest;
    PluginEntry {
        id: PluginId::try_new(id).expect("valid plugin id"),
        version: "0.0.1".into(),
        sha256: Sha256Digest::try_new(ZERO_DIGEST).expect("valid digest"),
        files: vec![],
        hooks: Default::default(),
    }
}

/// A candidate whose `owned_skill` is shipped by `plugin` and whose
/// `orphan_skill` has no recorded owner.
fn candidate(f: &Fixture, record_owner: bool) -> MarketplaceCandidate {
    let mut skill_owners = BTreeMap::new();
    if record_owner {
        skill_owners.insert(
            SkillId::try_new(&f.owned_skill).expect("valid skill id"),
            BTreeSet::from([PluginId::try_new(&f.plugin).expect("valid plugin id")]),
        );
    }
    MarketplaceCandidate {
        plugins: vec![plugin(&f.plugin)],
        skills: vec![skill(&f.owned_skill), skill(&f.orphan_skill)],
        skill_owners,
        marketplace_id: Some(MarketplaceId::new(&f.market)),
        ..MarketplaceCandidate::default()
    }
}

async fn visible(f: &Fixture, candidate: &MarketplaceCandidate, roles: &[&str]) -> Vec<String> {
    let repo = AccessControlRepository::new(&f.db).expect("repo");
    let roles: Vec<String> = roles.iter().map(|r| (*r).to_owned()).collect();
    let user = UserId::new("keep-sets-test-user");
    let sets = keep_sets(
        &repo,
        candidate,
        KeepSetsSubject {
            user_id: &user,
            roles: &roles,
            attributes: &NO_SUBJECT_ATTRIBUTES,
            dimensions: &[],
        },
    )
    .await
    .expect("keep_sets");
    let mut ids: Vec<String> = sets
        .skills
        .iter()
        .map(|id| id.as_str().to_owned())
        .collect();
    ids.sort();
    ids
}

// Why: this is the cascade's whole purpose. One rule on the plugin has to
// cover the skills it ships, or every bundle would need a rule per entry.
#[tokio::test]
async fn a_plugin_rule_reaches_the_skills_that_plugin_ships() {
    let f = setup().await;
    grant_role(&f, "plugin", &f.plugin, "engineer").await;

    let seen = visible(&f, &candidate(&f, true), &["engineer"]).await;

    assert!(
        seen.contains(&f.owned_skill),
        "the plugin's rule should reach its own skill; saw {seen:?}"
    );
    cleanup(&f).await;
}

#[tokio::test]
async fn a_role_the_plugin_rule_does_not_name_sees_nothing() {
    let f = setup().await;
    grant_role(&f, "plugin", &f.plugin, "engineer").await;

    let seen = visible(&f, &candidate(&f, true), &["contractor"]).await;

    assert!(
        !seen.contains(&f.owned_skill),
        "a role the rule does not name must not inherit the plugin's grant; saw {seen:?}"
    );
    cleanup(&f).await;
}

// Why: `skill_owners` is what carries the plugin relationship into the chain.
// Dropping it does not merely lose a link — the skill stops inheriting the
// plugin's grant entirely, which is how a bundle silently becomes invisible.
#[tokio::test]
async fn a_skill_with_no_recorded_owner_does_not_inherit_the_plugin_grant() {
    let f = setup().await;
    grant_role(&f, "plugin", &f.plugin, "engineer").await;

    let with_owner = visible(&f, &candidate(&f, true), &["engineer"]).await;
    let without_owner = visible(&f, &candidate(&f, false), &["engineer"]).await;

    assert!(
        with_owner.contains(&f.owned_skill),
        "control: with the owner recorded the skill inherits"
    );
    assert!(
        !without_owner.contains(&f.owned_skill),
        "without a recorded owner the skill must not inherit the plugin's grant; saw \
         {without_owner:?}"
    );
    cleanup(&f).await;
}

// Why: a rule on the skill itself is nearer than the plugin's, so it decides —
// including when it grants a role the plugin's rule does not.
#[tokio::test]
async fn a_rule_on_the_skill_itself_beats_the_plugin_rule() {
    let f = setup().await;
    grant_role(&f, "plugin", &f.plugin, "engineer").await;
    grant_role(&f, "skill", &f.owned_skill, "contractor").await;

    let seen = visible(&f, &candidate(&f, true), &["contractor"]).await;

    assert!(
        seen.contains(&f.owned_skill),
        "the skill's own rule is nearest and should decide; saw {seen:?}"
    );
    cleanup(&f).await;
}

#[tokio::test]
async fn the_plugin_itself_is_kept_when_its_rule_names_the_role() {
    let f = setup().await;
    grant_role(&f, "plugin", &f.plugin, "engineer").await;

    let repo = AccessControlRepository::new(&f.db).expect("repo");
    let user = UserId::new("keep-sets-test-user");
    let roles = vec!["engineer".to_owned()];
    let sets = keep_sets(
        &repo,
        &candidate(&f, true),
        KeepSetsSubject {
            user_id: &user,
            roles: &roles,
            attributes: &NO_SUBJECT_ATTRIBUTES,
            dimensions: &[],
        },
    )
    .await
    .expect("keep_sets");

    assert!(
        sets.plugins.iter().any(|p| p.as_str() == f.plugin),
        "the plugin its own rule names should be kept"
    );
    cleanup(&f).await;
}
