//! A routine, view, sequence, type, domain or index belongs to the first
//! member that creates it (core before install, then declarative schema
//! before migrations): an install member's re-create of such an object another
//! member creates is refused, so it never becomes the object's owner, and its
//! later alter or drop of the object stays refused (K-SEC6-1, K-SEC6-2).

use std::sync::Arc;

use systemprompt_database::SchemaMember;
use systemprompt_database::lifecycle::{
    MigrationOwnershipError, TableMutation, migration_ownership_findings,
};
use systemprompt_extension::{Migration, StandingTag};

use super::migration_edges::LogBuffer;
use super::migration_ownership::{all_pending, judged, only, unit};

const OWNER_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS mo_owned (id BIGINT PRIMARY KEY);
CREATE INDEX IF NOT EXISTS mo_owned_idx ON mo_owned (id);
CREATE TYPE mo_owned_kind AS ENUM ('a');
CREATE DOMAIN mo_owned_domain AS TEXT CONSTRAINT mo_owned_check CHECK (VALUE <> '');
CREATE SEQUENCE IF NOT EXISTS mo_owned_seq;
CREATE OR REPLACE VIEW mo_owned_view AS SELECT id FROM mo_owned;";

const OWNER_FN: &str = "CREATE OR REPLACE FUNCTION mo_owned_fn() RETURNS trigger LANGUAGE plpgsql \
                        AS $$BEGIN RETURN NEW; END$$;";

const PEER_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS mo_peer (id BIGINT PRIMARY KEY);
CREATE TYPE mo_peer_kind AS ENUM ('a');";

const RECREATE_FN_THEN_DROP: &str = "CREATE OR REPLACE FUNCTION public.mo_owned_fn() RETURNS \
                                     trigger LANGUAGE sql AS $$ SELECT NULL::trigger $$;
DROP FUNCTION public.mo_owned_fn() CASCADE;";

const RECREATE_INDEX_THEN_DROP: &str = "CREATE INDEX IF NOT EXISTS mo_owned_idx ON mo_peer (id);
DROP INDEX mo_owned_idx;";

fn owner() -> Arc<dyn SchemaMember> {
    unit(
        "mo-owner",
        StandingTag::Core,
        Some(OWNER_SCHEMA),
        vec![Migration::new(1, "function", OWNER_FN)],
        vec![],
    )
}

fn peer(standing: StandingTag, sql: &'static str) -> Arc<dyn SchemaMember> {
    unit(
        "mo-peer",
        standing,
        Some(PEER_SCHEMA),
        vec![Migration::new(1, "probe", sql)],
        vec![],
    )
}

fn refusals(members: &[Arc<dyn SchemaMember>]) -> Vec<(TableMutation, String, String)> {
    migration_ownership_findings(only(members), &[], &all_pending)
        .into_iter()
        .map(|finding| match finding {
            MigrationOwnershipError::ForeignObject {
                statement,
                object,
                owner,
                member,
                ..
            } => {
                assert_eq!(member.as_str(), "mo-peer");
                (statement, object, owner.as_str().to_owned())
            },
            other => panic!("expected a foreign-object refusal, got {other:?}"),
        })
        .collect()
}

fn refused(kind: TableMutation, object: &str) -> (TableMutation, String, String) {
    (kind, object.to_owned(), "mo-owner".to_owned())
}

fn admitted(members: &[Arc<dyn SchemaMember>]) {
    judged(members).unwrap_or_else(|e| panic!("negative control is admitted: {e}"));
}

#[test]
fn an_install_recreating_a_core_function_then_dropping_it_is_refused_twice() {
    assert_eq!(
        refusals(&[owner(), peer(StandingTag::Install, RECREATE_FN_THEN_DROP)]),
        vec![
            refused(TableMutation::CreateObject, "public.mo_owned_fn"),
            refused(TableMutation::DropObject, "public.mo_owned_fn"),
        ]
    );
    admitted(&[owner(), peer(StandingTag::Core, RECREATE_FN_THEN_DROP)]);
    admitted(&[unit(
        "mo-owner",
        StandingTag::Install,
        Some(OWNER_SCHEMA),
        vec![
            Migration::new(1, "function", OWNER_FN),
            Migration::new(2, "probe", RECREATE_FN_THEN_DROP),
        ],
        vec![],
    )]);
}

#[test]
fn an_install_naming_a_core_index_if_not_exists_then_dropping_it_is_refused_twice() {
    assert_eq!(
        refusals(&[
            owner(),
            peer(StandingTag::Install, RECREATE_INDEX_THEN_DROP)
        ]),
        vec![
            refused(TableMutation::CreateIndex, "public.mo_owned_idx"),
            refused(TableMutation::DropIndex, "public.mo_owned_idx"),
        ]
    );
    admitted(&[owner(), peer(StandingTag::Core, RECREATE_INDEX_THEN_DROP)]);
    admitted(&[
        owner(),
        peer(
            StandingTag::Install,
            "CREATE INDEX IF NOT EXISTS mo_peer_idx ON mo_peer (id);\nDROP INDEX mo_peer_idx;",
        ),
    ]);
}

#[test]
fn the_first_creator_owns_an_object_whatever_the_composition_order() {
    assert_eq!(
        refusals(&[peer(StandingTag::Install, RECREATE_FN_THEN_DROP), owner()]),
        vec![
            refused(TableMutation::CreateObject, "public.mo_owned_fn"),
            refused(TableMutation::DropObject, "public.mo_owned_fn"),
        ]
    );
}

#[test]
fn an_install_recreating_a_core_view_sequence_type_or_domain_is_refused() {
    let cases: [(&'static str, &str); 4] = [
        (
            "CREATE OR REPLACE VIEW mo_owned_view AS SELECT id FROM mo_peer;",
            "public.mo_owned_view",
        ),
        (
            "CREATE SEQUENCE IF NOT EXISTS mo_owned_seq;",
            "public.mo_owned_seq",
        ),
        (
            "CREATE TYPE mo_owned_kind AS ENUM ('b');",
            "public.mo_owned_kind",
        ),
        (
            "CREATE DOMAIN mo_owned_domain AS TEXT;",
            "public.mo_owned_domain",
        ),
    ];
    for (sql, object) in cases {
        assert_eq!(
            refusals(&[owner(), peer(StandingTag::Install, sql)]),
            vec![refused(TableMutation::CreateObject, object)],
            "{sql}"
        );
        admitted(&[owner(), peer(StandingTag::Core, sql)]);
    }
    admitted(&[
        owner(),
        peer(
            StandingTag::Install,
            "CREATE OR REPLACE VIEW mo_peer_view AS SELECT id FROM mo_peer;",
        ),
    ]);
}

#[test]
fn an_install_schema_recreating_a_core_function_is_refused() {
    let schema_peer = |standing| {
        unit(
            "mo-peer",
            standing,
            Some(
                "CREATE OR REPLACE FUNCTION mo_owned_fn() RETURNS trigger LANGUAGE plpgsql AS \
                 $$BEGIN RETURN NULL; END$$;",
            ),
            vec![],
            vec![],
        )
    };
    let findings = migration_ownership_findings(
        only(&[owner(), schema_peer(StandingTag::Install)]),
        &[],
        &all_pending,
    );
    assert!(
        matches!(
            findings.as_slice(),
            [MigrationOwnershipError::ForeignObject { migration, statement: TableMutation::CreateObject, object, owner, .. }]
                if migration == "schema" && object == "public.mo_owned_fn" && owner.as_str() == "mo-owner"
        ),
        "{findings:?}"
    );
    admitted(&[owner(), schema_peer(StandingTag::Core)]);
}

#[test]
fn an_install_altering_a_core_routine_sequence_view_type_or_domain_is_refused() {
    let cases: [(&'static str, &'static str, &str); 6] = [
        (
            "ALTER FUNCTION mo_owned_fn() SECURITY DEFINER;",
            "ALTER FUNCTION mo_peer_fn() SECURITY DEFINER;",
            "public.mo_owned_fn",
        ),
        (
            "ALTER SEQUENCE mo_owned_seq RESTART;",
            "ALTER SEQUENCE mo_peer_seq RESTART;",
            "public.mo_owned_seq",
        ),
        (
            "ALTER VIEW mo_owned_view SET (security_barrier = true);",
            "ALTER VIEW mo_peer_view SET (security_barrier = true);",
            "public.mo_owned_view",
        ),
        (
            "ALTER VIEW mo_owned_view RENAME TO mo_taken_view;",
            "ALTER VIEW mo_peer_view RENAME TO mo_kept_view;",
            "public.mo_owned_view",
        ),
        (
            "ALTER TYPE mo_owned_kind ADD VALUE 'b';",
            "ALTER TYPE mo_peer_kind ADD VALUE 'b';",
            "public.mo_owned_kind",
        ),
        (
            "ALTER DOMAIN mo_owned_domain DROP CONSTRAINT mo_owned_check;",
            "ALTER DOMAIN mo_peer_domain DROP CONSTRAINT mo_peer_check;",
            "public.mo_owned_domain",
        ),
    ];
    for (foreign, own, object) in cases {
        assert_eq!(
            refusals(&[owner(), peer(StandingTag::Install, foreign)]),
            vec![refused(TableMutation::AlterObject, object)],
            "{foreign}"
        );
        admitted(&[owner(), peer(StandingTag::Core, foreign)]);
        admitted(&[
            owner(),
            unit(
                "mo-peer",
                StandingTag::Install,
                Some(
                    "CREATE TABLE IF NOT EXISTS mo_peer (id BIGINT PRIMARY KEY);
CREATE TYPE mo_peer_kind AS ENUM ('a');
CREATE DOMAIN mo_peer_domain AS TEXT CONSTRAINT mo_peer_check CHECK (VALUE <> '');
CREATE SEQUENCE IF NOT EXISTS mo_peer_seq;
CREATE OR REPLACE VIEW mo_peer_view AS SELECT id FROM mo_peer;
CREATE OR REPLACE FUNCTION mo_peer_fn() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RETURN NEW; END$$;",
                ),
                vec![Migration::new(1, "probe", own)],
                vec![],
            ),
        ]);
    }
}

#[test]
fn an_install_extension_routine_call_in_a_write_and_cascade_are_logged() {
    let cases = [
        (
            "CREATE EXTENSION IF NOT EXISTS pgcrypto;",
            "statement=CREATE EXTENSION",
        ),
        (
            "INSERT INTO mo_peer (id) SELECT mo_peer_next();",
            "statement=call of a routine",
        ),
        (
            "UPDATE mo_peer SET id = mo_peer_next();",
            "statement=call of a routine",
        ),
        (
            "DELETE FROM mo_peer WHERE mo_peer_gone(id);",
            "statement=call of a routine",
        ),
        (
            "DROP TYPE mo_peer_kind CASCADE;",
            "statement=DROP ... CASCADE",
        ),
    ];
    for (sql, line) in cases {
        let logs = LogBuffer::default();
        let outcome = {
            let _guard = logs.install();
            judged(&[owner(), peer(StandingTag::Install, sql)])
        };
        outcome.unwrap_or_else(|e| panic!("{sql} is logged, not refused: {e}"));
        let text = logs.text();
        assert!(text.contains(line), "{sql}: {text}");
    }
    let logs = LogBuffer::default();
    {
        let _guard = logs.install();
        admitted(&[
            owner(),
            peer(StandingTag::Install, "INSERT INTO mo_peer (id) VALUES (1);"),
        ]);
    }
    assert!(
        !logs.text().contains("call of a routine"),
        "{}",
        logs.text()
    );
}
