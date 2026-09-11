//! How the two rule/entity tags cross the Postgres boundary.
//!
//! `rule_type` and `entity_type` are stored as plain text, so the vocabulary is
//! policed in Rust on the way back out. The two tags deliberately differ:
//! `RuleType` is an open vocabulary an extension may widen, while `EntityKind`
//! is closed and must refuse a slug core does not know.

use std::str::FromStr;

use systemprompt_security::authz::{EntityKind, RuleType};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

async fn pool_or_skip() -> Option<sqlx::PgPool> {
    let url = fixture_database_url().ok()?;
    let db = fixture_db_pool(&url).await.ok()?;
    let pg = db.write_pool_arc().ok()?;
    Some((*pg).clone())
}

#[tokio::test]
async fn the_core_rule_types_survive_a_database_round_trip_unchanged() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };

    for expected in [RuleType::USER, RuleType::ROLE] {
        let decoded: RuleType = sqlx::query_scalar("SELECT $1::text")
            .bind(expected.clone())
            .fetch_one(&pool)
            .await
            .expect("a rule type binds and decodes as text");

        assert_eq!(
            decoded, expected,
            "a core rule type must come back as the same tag it was written as"
        );
    }
}

#[tokio::test]
async fn an_extension_minted_rule_type_round_trips_without_core_interpreting_it() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let minted = RuleType::extension("cost_centre").expect("a well-formed extension slug");

    let decoded: RuleType = sqlx::query_scalar("SELECT $1::text")
        .bind(minted.clone())
        .fetch_one(&pool)
        .await
        .expect("an extension rule type is storable");

    assert_eq!(decoded, minted);
    assert_eq!(decoded.as_str(), "cost_centre");
}

#[tokio::test]
async fn a_rule_type_core_does_not_recognise_decodes_as_data_rather_than_failing() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };

    let decoded: RuleType = sqlx::query_scalar("SELECT 'clearance_level'::text")
        .fetch_one(&pool)
        .await
        .expect("an unrecognised dimension is data, not a decode error");

    assert_eq!(decoded.as_str(), "clearance_level");
    assert_ne!(decoded, RuleType::USER);
    assert_ne!(decoded, RuleType::ROLE);
}

#[tokio::test]
async fn every_entity_kind_round_trips_through_its_stored_text() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };

    for expected in EntityKind::ALL.iter().copied() {
        let decoded: EntityKind = sqlx::query_scalar("SELECT $1::text")
            .bind(expected)
            .fetch_one(&pool)
            .await
            .unwrap_or_else(|e| panic!("{expected} must round-trip: {e}"));

        assert_eq!(decoded, expected);
    }
}

#[tokio::test]
async fn an_entity_kind_outside_the_closed_vocabulary_fails_to_decode() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };

    let error = sqlx::query_scalar::<_, EntityKind>("SELECT 'wormhole'::text")
        .fetch_one(&pool)
        .await
        .expect_err("an entity type core cannot resolve must not be handed to the resolver");

    assert!(
        error.to_string().contains("wormhole"),
        "the failure must name the unknown entity type: {error}"
    );
    assert!(
        EntityKind::from_str("wormhole").is_err(),
        "the decode rejection and the parser must agree"
    );
}
