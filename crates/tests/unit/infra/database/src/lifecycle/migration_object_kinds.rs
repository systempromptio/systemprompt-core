//! Every statement kind that changes a named object is judged against the
//! object's owner, whatever spelling names it: a table form (`ALTER TABLE`,
//! `RENAME`, `SET SCHEMA`, `OWNER TO`) of a view, materialized view,
//! sequence or index, the object's own form, a type attribute or domain
//! constraint alter, and a trigger create, rename or drop (K-SEC7-1/2/3/5).
//! Each row is refused for an install member, admitted for a core member and
//! admitted when the owner runs it, so removing any one refusal turns its row
//! red. Ownership ranks a record first, then core before install, then a
//! declarative schema before a migration, then composition order; a held-back
//! member owns what it creates and its schema is not judged; a member outside
//! the composition owns what its records name (K-SEC7-4). An install
//! member's declarative schema runs through the same judgement as a
//! migration, every row of the table included; every relation a statement
//! changes is judged (`INHERITS`, `PARTITION OF`, `INHERIT`, `ATTACH` and
//! `DETACH PARTITION`), while a read or reference is not; a grant on any
//! object is judged; a statement kind the reader does not classify is refused
//! for an install and logged for core (K-SEC8-1/3/6).

use std::sync::Arc;

use systemprompt_database::lifecycle::{
    MigrationMembers, MigrationOwnershipError, ObjectKind, Recorded, RecordedObject,
    migration_ownership_findings,
};
use systemprompt_database::{SchemaMember, SchemaUnit};
use systemprompt_extension::{ExtensionMetadata, Migration, SchemaDefinition, StandingTag};
use systemprompt_identifiers::ExtensionId;

use super::migration_edges::LogBuffer;
use super::migration_ownership::{all_pending, judged, only, unit};

const OWNER_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS ko_t (id INT PRIMARY KEY);
CREATE INDEX IF NOT EXISTS ko_i ON ko_t (id);
CREATE OR REPLACE VIEW ko_v AS SELECT id FROM ko_t;
CREATE MATERIALIZED VIEW IF NOT EXISTS ko_mv AS SELECT id FROM ko_t;
CREATE SEQUENCE IF NOT EXISTS ko_s;
CREATE OR REPLACE FUNCTION ko_f() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RETURN NEW; END$$;
CREATE TYPE ko_c AS (a INT);
CREATE TYPE ko_e AS ENUM ('a');
CREATE TYPE ko_r AS RANGE (subtype = int4);
CREATE DOMAIN ko_d AS TEXT CONSTRAINT ko_d_check CHECK (VALUE <> '');
CREATE TABLE IF NOT EXISTS ko_ctas AS SELECT 1 AS x;
CREATE OR REPLACE TRIGGER ko_trg BEFORE UPDATE ON ko_t FOR EACH ROW EXECUTE FUNCTION ko_f();";

const PEER_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS kp_t (id INT PRIMARY KEY);";

const CASES: &[(&str, &[&str])] = &[
    (
        "ALTER VIEW ko_v SET SCHEMA mine;",
        &["object SET SCHEMA public.ko_v"],
    ),
    (
        "ALTER MATERIALIZED VIEW ko_mv SET SCHEMA mine;",
        &["object SET SCHEMA public.ko_mv"],
    ),
    (
        "ALTER SEQUENCE ko_s SET SCHEMA mine;",
        &["object SET SCHEMA public.ko_s"],
    ),
    (
        "ALTER TABLE ko_v SET SCHEMA mine;",
        &["object SET SCHEMA public.ko_v"],
    ),
    (
        "ALTER TABLE ko_s SET SCHEMA mine;",
        &["object SET SCHEMA public.ko_s"],
    ),
    (
        "ALTER TABLE ko_v RENAME TO x;",
        &["object RENAME public.ko_v"],
    ),
    (
        "ALTER TABLE ko_mv RENAME TO x;",
        &["object RENAME public.ko_mv"],
    ),
    (
        "ALTER TABLE ko_s RENAME TO x;",
        &["object RENAME public.ko_s"],
    ),
    (
        "ALTER TABLE ko_i RENAME TO x;",
        &["object RENAME public.ko_i"],
    ),
    (
        "ALTER TABLE ko_v OWNER TO someone;",
        &["object ALTER TABLE public.ko_v"],
    ),
    (
        "ALTER TABLE ko_s OWNER TO someone;",
        &["object ALTER TABLE public.ko_s"],
    ),
    (
        "ALTER VIEW ko_v RENAME COLUMN id TO x;",
        &["object RENAME public.ko_v"],
    ),
    (
        "ALTER VIEW ko_v OWNER TO someone;",
        &["object ALTER public.ko_v"],
    ),
    (
        "ALTER SEQUENCE ko_s OWNER TO someone;",
        &["object ALTER public.ko_s"],
    ),
    (
        "ALTER SEQUENCE ko_s RENAME TO x;",
        &["object ALTER public.ko_s"],
    ),
    (
        "ALTER MATERIALIZED VIEW ko_mv RENAME TO x;",
        &["object ALTER public.ko_mv"],
    ),
    (
        "ALTER MATERIALIZED VIEW ko_mv OWNER TO someone;",
        &["object ALTER public.ko_mv"],
    ),
    (
        "ALTER INDEX ko_i RENAME TO x;",
        &["object ALTER INDEX public.ko_i"],
    ),
    (
        "ALTER INDEX ko_i SET (fillfactor = 50);",
        &["object ALTER INDEX public.ko_i"],
    ),
    (
        "ALTER FUNCTION ko_f() RENAME TO x;",
        &["object ALTER public.ko_f"],
    ),
    (
        "ALTER FUNCTION ko_f() SET SCHEMA mine;",
        &["object SET SCHEMA public.ko_f"],
    ),
    (
        "ALTER FUNCTION ko_f() OWNER TO someone;",
        &["object ALTER public.ko_f"],
    ),
    (
        "ALTER TYPE ko_c ADD ATTRIBUTE b INT;",
        &["object ALTER public.ko_c"],
    ),
    (
        "ALTER TYPE ko_c DROP ATTRIBUTE a CASCADE;",
        &["object ALTER public.ko_c"],
    ),
    (
        "ALTER TYPE ko_c ALTER ATTRIBUTE a TYPE BIGINT;",
        &["object ALTER public.ko_c"],
    ),
    (
        "ALTER TYPE ko_c RENAME ATTRIBUTE a TO b;",
        &["object ALTER public.ko_c"],
    ),
    (
        "ALTER TYPE ko_e RENAME TO x;",
        &["object ALTER public.ko_e"],
    ),
    (
        "ALTER TYPE ko_e SET SCHEMA mine;",
        &["object SET SCHEMA public.ko_e"],
    ),
    (
        "ALTER TYPE ko_e OWNER TO someone;",
        &["object ALTER public.ko_e"],
    ),
    (
        "ALTER TYPE ko_r RENAME TO x;",
        &["object ALTER public.ko_r"],
    ),
    (
        "ALTER DOMAIN ko_d RENAME CONSTRAINT ko_d_check TO x;",
        &["object ALTER public.ko_d"],
    ),
    (
        "ALTER DOMAIN ko_d RENAME TO x;",
        &["object ALTER public.ko_d"],
    ),
    (
        "ALTER DOMAIN ko_d SET SCHEMA mine;",
        &["object SET SCHEMA public.ko_d"],
    ),
    (
        "ALTER DOMAIN ko_d OWNER TO someone;",
        &["object ALTER public.ko_d"],
    ),
    ("DROP VIEW ko_v;", &["object DROP public.ko_v"]),
    (
        "DROP MATERIALIZED VIEW ko_mv;",
        &["object DROP public.ko_mv"],
    ),
    ("DROP SEQUENCE ko_s;", &["object DROP public.ko_s"]),
    ("DROP INDEX ko_i;", &["object DROP INDEX public.ko_i"]),
    ("DROP FUNCTION ko_f();", &["object DROP public.ko_f"]),
    ("DROP TYPE ko_c;", &["object DROP public.ko_c"]),
    ("DROP TYPE ko_r;", &["object DROP public.ko_r"]),
    ("DROP DOMAIN ko_d;", &["object DROP public.ko_d"]),
    (
        "CREATE OR REPLACE VIEW ko_v AS SELECT 1 AS id;",
        &["object CREATE public.ko_v"],
    ),
    (
        "CREATE MATERIALIZED VIEW ko_mv AS SELECT 1 AS id;",
        &["object CREATE public.ko_mv"],
    ),
    (
        "CREATE TYPE ko_c AS (a INT);",
        &["object CREATE public.ko_c"],
    ),
    (
        "CREATE TYPE ko_r AS RANGE (subtype = int4);",
        &["object CREATE public.ko_r"],
    ),
    (
        "CREATE INDEX IF NOT EXISTS ko_i ON kp_t (id);",
        &["object CREATE INDEX public.ko_i"],
    ),
    (
        "CREATE OR REPLACE TRIGGER ko_trg BEFORE UPDATE ON ko_t FOR EACH ROW EXECUTE FUNCTION kp_f();",
        &["object CREATE TRIGGER ko_trg ON public.ko_t"],
    ),
    (
        "DROP TRIGGER ko_trg ON ko_t;",
        &["object DROP TRIGGER ko_trg ON public.ko_t"],
    ),
    (
        "ALTER TRIGGER ko_trg ON ko_t RENAME TO x;",
        &[
            "table RENAME public.ko_t",
            "object RENAME ko_trg ON public.ko_t",
        ],
    ),
    (
        "CREATE TABLE ko_ctas AS SELECT 2 AS x;",
        &["shared public.ko_ctas"],
    ),
    (
        "ALTER TABLE ko_ctas ADD COLUMN y INT;",
        &["table ALTER TABLE public.ko_ctas"],
    ),
    ("DROP TABLE ko_ctas;", &["table DROP TABLE public.ko_ctas"]),
    ("SELECT 2 AS x INTO ko_ctas;", &["shared public.ko_ctas"]),
    ("DROP VIEW IF EXISTS ko_v;", &["object DROP public.ko_v"]),
    (
        "DROP MATERIALIZED VIEW IF EXISTS ko_mv;",
        &["object DROP public.ko_mv"],
    ),
    (
        "DROP INDEX IF EXISTS ko_i;",
        &["object DROP INDEX public.ko_i"],
    ),
    (
        "DROP TRIGGER IF EXISTS ko_trg ON ko_t;",
        &["object DROP TRIGGER ko_trg ON public.ko_t"],
    ),
    (
        "CREATE RULE kp_rule AS ON INSERT TO ko_t DO INSTEAD NOTHING;",
        &["table CREATE RULE public.ko_t"],
    ),
    (
        "CREATE POLICY kp_policy ON ko_t USING (true);",
        &["table CREATE POLICY public.ko_t"],
    ),
    (
        "ALTER TABLE ko_t ENABLE ROW LEVEL SECURITY;",
        &["table ALTER TABLE public.ko_t"],
    ),
    (
        "CREATE TABLE kp_child (x INT) INHERITS (ko_t);",
        &["table INHERITS/INHERIT/NO INHERIT public.ko_t"],
    ),
    (
        "CREATE TABLE kp_part PARTITION OF ko_t FOR VALUES FROM (1) TO (2);",
        &["table PARTITION OF/ATTACH/DETACH PARTITION public.ko_t"],
    ),
    (
        "ALTER TABLE kp_t INHERIT ko_t;",
        &["table INHERITS/INHERIT/NO INHERIT public.ko_t"],
    ),
    (
        "ALTER TABLE kp_t NO INHERIT ko_t;",
        &["table INHERITS/INHERIT/NO INHERIT public.ko_t"],
    ),
    (
        "ALTER TABLE kp_t ATTACH PARTITION ko_t FOR VALUES FROM (1) TO (2);",
        &["table PARTITION OF/ATTACH/DETACH PARTITION public.ko_t"],
    ),
    (
        "ALTER TABLE kp_t DETACH PARTITION ko_t;",
        &["table PARTITION OF/ATTACH/DETACH PARTITION public.ko_t"],
    ),
    (
        "GRANT EXECUTE ON FUNCTION ko_f() TO PUBLIC;",
        &["object GRANT public.ko_f"],
    ),
    (
        "REVOKE ALL ON FUNCTION ko_f() FROM PUBLIC;",
        &["object REVOKE public.ko_f"],
    ),
    (
        "GRANT USAGE ON SEQUENCE ko_s TO PUBLIC;",
        &["object GRANT public.ko_s"],
    ),
    (
        "GRANT USAGE ON TYPE ko_e TO PUBLIC;",
        &["object GRANT public.ko_e"],
    ),
    (
        "GRANT USAGE ON DOMAIN ko_d TO PUBLIC;",
        &["object GRANT public.ko_d"],
    ),
];

const READS: &[&str] = &[
    "CREATE TABLE kp_like (LIKE ko_t INCLUDING ALL);",
    "CREATE TABLE kp_ref (id INT REFERENCES ko_t (id));",
    "ALTER TABLE kp_t ADD CONSTRAINT kp_fk FOREIGN KEY (id) REFERENCES ko_t (id);",
    "CREATE VIEW kp_v AS SELECT id FROM ko_v;",
    "SELECT id INTO kp_copy FROM ko_t;",
    "COMMENT ON VIEW ko_v IS 'x';",
];

const UNCLASSIFIED: &[(&str, &str)] = &[
    ("GRANT USAGE ON LANGUAGE sql TO PUBLIC;", "GrantStmt"),
    ("ALTER COLLATION kp_c RENAME TO kp_d;", "RenameStmt"),
    (
        "ALTER DATABASE kp_db SET work_mem = '1MB';",
        "AlterDatabaseSetStmt",
    ),
    ("CREATE ROLE kp_role;", "CreateRoleStmt"),
];

fn leak(sql: String) -> &'static str {
    Box::leak(sql.into_boxed_str())
}

fn member(
    id: &'static str,
    standing: StandingTag,
    schema: Option<&'static str>,
    sql: Option<&'static str>,
) -> Arc<dyn SchemaMember> {
    unit(
        id,
        standing,
        schema,
        sql.map(|sql| vec![Migration::new(1, "probe", sql)])
            .unwrap_or_default(),
        vec![],
    )
}

fn owner(standing: StandingTag, sql: Option<&'static str>) -> Arc<dyn SchemaMember> {
    member("ko-owner", standing, Some(OWNER_SCHEMA), sql)
}

fn peer(standing: StandingTag, sql: &'static str) -> Arc<dyn SchemaMember> {
    member("ko-peer", standing, Some(PEER_SCHEMA), Some(sql))
}

fn described(findings: Vec<MigrationOwnershipError>) -> Vec<String> {
    findings
        .into_iter()
        .map(|finding| match finding {
            MigrationOwnershipError::ForeignObject {
                statement, object, ..
            } => {
                format!("object {statement} {object}")
            },
            MigrationOwnershipError::ForeignTable {
                statement, table, ..
            } => {
                format!("table {statement} {table}")
            },
            MigrationOwnershipError::SharedTable { table, .. } => format!("shared {table}"),
            other => format!("{other}"),
        })
        .collect()
}

fn findings_of(members: &[Arc<dyn SchemaMember>]) -> Vec<String> {
    described(migration_ownership_findings(
        only(members),
        &Recorded::NONE,
        &all_pending,
    ))
}

#[test]
fn every_statement_kind_on_another_members_object_is_refused_for_an_install() {
    for (sql, expected) in CASES {
        assert_eq!(
            findings_of(&[
                owner(StandingTag::Core, None),
                peer(StandingTag::Install, sql)
            ]),
            expected
                .iter()
                .map(|line| (*line).to_owned())
                .collect::<Vec<_>>(),
            "{sql}"
        );
    }
}

#[test]
fn every_statement_kind_is_admitted_for_a_core_member_and_for_the_owner() {
    for (sql, _) in CASES {
        let core = findings_of(&[owner(StandingTag::Core, None), peer(StandingTag::Core, sql)]);
        let shared_table_only = core
            .iter()
            .all(|line| line.starts_with("shared ") || line.starts_with("table "));
        assert!(core.is_empty() || shared_table_only, "{sql}: {core:?}");
        assert!(
            findings_of(&[owner(StandingTag::Install, Some(sql))]).is_empty(),
            "the owner's own statement is admitted: {sql}"
        );
    }
}

#[test]
fn every_statement_kind_in_an_install_schema_is_judged_like_a_migration() {
    for (sql, expected) in CASES {
        let schema = leak(format!("{PEER_SCHEMA}\n{sql}"));
        assert_eq!(
            findings_of(&[
                owner(StandingTag::Core, None),
                member("ko-peer", StandingTag::Install, Some(schema), None),
            ]),
            expected
                .iter()
                .map(|line| (*line).to_owned())
                .collect::<Vec<_>>(),
            "schema: {sql}"
        );
        let own = leak(format!("{OWNER_SCHEMA}\n{sql}"));
        assert!(
            findings_of(&[member("ko-owner", StandingTag::Install, Some(own), None)]).is_empty(),
            "negative control, the owner's own schema statement is admitted: {sql}"
        );
    }
}

#[test]
fn a_core_members_schema_is_not_judged() {
    let schema = leak(format!("{PEER_SCHEMA}\nDROP VIEW IF EXISTS ko_v;"));
    assert!(
        findings_of(&[
            owner(StandingTag::Core, None),
            member("ko-peer", StandingTag::Core, Some(schema), None),
        ])
        .is_empty()
    );
}

#[test]
fn reading_or_referencing_another_members_relation_is_admitted() {
    for sql in READS {
        assert!(
            findings_of(&[
                owner(StandingTag::Core, None),
                peer(StandingTag::Install, sql)
            ])
            .is_empty(),
            "{sql}"
        );
    }
}

#[test]
fn an_unclassified_statement_kind_is_refused_for_an_install_and_logged_for_core() {
    for (sql, kind) in UNCLASSIFIED {
        assert_eq!(
            findings_of(&[
                owner(StandingTag::Core, None),
                peer(StandingTag::Install, sql)
            ]),
            vec![format!(
                "install member ko-peer migration 001_probe: a unclassified {kind} statement \
                 hides what it changes"
            )],
            "{sql}"
        );
        let logs = LogBuffer::default();
        {
            let _guard = logs.install();
            assert!(
                findings_of(&[owner(StandingTag::Core, None), peer(StandingTag::Core, sql)])
                    .is_empty(),
                "{sql}"
            );
        }
        assert!(
            logs.text()
                .contains(&format!("statement=unclassified {kind}")),
            "{sql}: {}",
            logs.text()
        );
    }
    assert!(
        findings_of(&[
            owner(StandingTag::Core, None),
            peer(StandingTag::Install, "DO $$ BEGIN END $$;")
        ])
        .is_empty(),
        "negative control: a DO block is logged, not refused"
    );
}

#[test]
fn a_core_members_statement_on_another_members_object_is_logged() {
    let logs = LogBuffer::default();
    {
        let _guard = logs.install();
        assert!(
            findings_of(&[
                owner(StandingTag::Core, None),
                peer(
                    StandingTag::Core,
                    "ALTER VIEW ko_v SET SCHEMA mine;
DROP TRIGGER ko_trg ON ko_t;"
                ),
            ])
            .is_empty()
        );
    }
    let text = logs.text();
    assert!(text.contains("object=public.ko_v"), "{text}");
    assert!(text.contains("object=ko_trg ON public.ko_t"), "{text}");
}

#[test]
fn a_new_trigger_on_another_members_table_is_logged_not_refused() {
    let logs = LogBuffer::default();
    let outcome = {
        let _guard = logs.install();
        judged(&[
            owner(StandingTag::Core, None),
            peer(
                StandingTag::Install,
                "CREATE TRIGGER kp_trg AFTER INSERT ON ko_t FOR EACH ROW EXECUTE FUNCTION kp_f();
DROP TRIGGER kp_trg ON ko_t;",
            ),
        ])
    };
    outcome.unwrap_or_else(|e| panic!("a new trigger is logged: {e}"));
    let text = logs.text();
    assert!(text.contains("statement=CREATE TRIGGER"), "{text}");
    assert!(text.contains("statement=DROP TRIGGER"), "{text}");
}

#[test]
fn a_merge_calling_a_routine_is_logged() {
    let logs = LogBuffer::default();
    {
        let _guard = logs.install();
        judged(&[
            owner(StandingTag::Core, None),
            peer(
                StandingTag::Install,
                "MERGE INTO kp_t t USING (SELECT 1 AS id) s ON t.id = s.id \
                 WHEN NOT MATCHED THEN INSERT (id) VALUES (kp_next());",
            ),
        ])
        .unwrap_or_else(|e| panic!("a MERGE routine call is logged: {e}"));
    }
    assert!(
        logs.text().contains("statement=call of a routine"),
        "{}",
        logs.text()
    );
}

const FN_A: &str = "CREATE OR REPLACE FUNCTION kz_f() RETURNS INT LANGUAGE sql AS $$ SELECT 1 $$;";

fn creator(id: &'static str, standing: StandingTag, schema: bool) -> Arc<dyn SchemaMember> {
    if schema {
        member(id, standing, Some(FN_A), None)
    } else {
        member(id, standing, None, Some(FN_A))
    }
}

fn owners(members: MigrationMembers<'_>) -> Vec<(String, String)> {
    migration_ownership_findings(members, &Recorded::NONE, &all_pending)
        .into_iter()
        .map(|finding| match finding {
            MigrationOwnershipError::ForeignObject { member, owner, .. } => {
                (member.as_str().to_owned(), owner.as_str().to_owned())
            },
            other => panic!("expected a foreign-object refusal, got {other:?}"),
        })
        .collect()
}

#[test]
fn two_installs_creating_one_object_rank_by_composition_order() {
    let a = creator("kz-a", StandingTag::Install, false);
    let b = creator("kz-b", StandingTag::Install, false);
    assert_eq!(
        owners(only(&[a.clone(), b.clone()])),
        vec![("kz-b".to_owned(), "kz-a".to_owned())]
    );
    assert_eq!(
        owners(only(&[b, a])),
        vec![("kz-a".to_owned(), "kz-b".to_owned())]
    );
}

#[test]
fn a_held_back_member_ranks_by_its_composition_position() {
    let held = creator("kz-held", StandingTag::Install, false);
    let judged_member = creator("kz-judged", StandingTag::Install, false);
    let composition = [held.clone(), judged_member.clone()];
    assert_eq!(
        owners(MigrationMembers::new(
            &composition,
            std::slice::from_ref(&held)
        )),
        vec![("kz-judged".to_owned(), "kz-held".to_owned())]
    );
    let reversed = [judged_member, held.clone()];
    assert!(
        owners(MigrationMembers::new(
            &reversed,
            std::slice::from_ref(&held)
        ))
        .is_empty(),
        "the earlier judged member owns, and the held-back member is not judged"
    );
}

#[test]
fn a_held_back_members_schema_is_not_judged() {
    let core = creator("kz-core", StandingTag::Core, false);
    let held = creator("kz-held", StandingTag::Install, true);
    let composition = [core.clone(), held.clone()];
    assert!(
        owners(MigrationMembers::new(
            &composition,
            std::slice::from_ref(&held)
        ))
        .is_empty()
    );
    assert_eq!(
        owners(only(&composition)),
        vec![("kz-held".to_owned(), "kz-core".to_owned())],
        "negative control: judged, its schema's create of the core function is refused"
    );
}

#[test]
fn a_core_schema_claim_outranks_an_install_migration_claim() {
    assert_eq!(
        owners(only(&[
            creator("kz-install", StandingTag::Install, false),
            creator("kz-core", StandingTag::Core, true),
        ])),
        vec![("kz-install".to_owned(), "kz-core".to_owned())]
    );
}

#[test]
fn a_core_migration_table_outranks_an_install_schema_table() {
    let findings = described(migration_ownership_findings(
        only(&[
            member(
                "kz-install",
                StandingTag::Install,
                Some("CREATE TABLE kz_t (id INT);"),
                None,
            ),
            member(
                "kz-core",
                StandingTag::Core,
                None,
                Some("CREATE TABLE kz_t (id INT);"),
            ),
        ]),
        &Recorded::NONE,
        &all_pending,
    ));
    assert_eq!(findings, vec!["shared public.kz_t".to_owned()]);
}

fn gone(kind: ObjectKind, name: &str) -> RecordedObject {
    RecordedObject {
        kind,
        name: name.to_owned(),
        owner: ExtensionId::new("kz-gone"),
        standing: StandingTag::Install,
    }
}

#[test]
fn a_member_outside_the_composition_owns_what_its_records_name() {
    let peer = member(
        "kz-peer",
        StandingTag::Install,
        Some("CREATE TABLE kz_t (id INT);"),
        Some(
            "DROP FUNCTION kz_gone_fn();
DROP TRIGGER kz_gone_trg ON kz_t;
DROP INDEX kz_gone_idx;",
        ),
    );
    let records = [
        gone(ObjectKind::Object, "public.kz_gone_fn"),
        gone(ObjectKind::Trigger, "kz_gone_trg ON public.kz_t"),
        gone(ObjectKind::Index, "public.kz_gone_idx"),
    ];
    let members = [peer];
    let findings = described(migration_ownership_findings(
        only(&members),
        &Recorded {
            tables: &[],
            objects: &records,
        },
        &all_pending,
    ));
    assert_eq!(
        findings,
        vec![
            "object DROP public.kz_gone_fn".to_owned(),
            "object DROP TRIGGER kz_gone_trg ON public.kz_t".to_owned(),
            "object DROP INDEX public.kz_gone_idx".to_owned(),
        ]
    );
    assert!(
        migration_ownership_findings(only(&members), &Recorded::NONE, &all_pending).is_empty(),
        "negative control: without the records nothing owns them"
    );
}

fn retiring_creator(retired: bool) -> Arc<dyn SchemaMember> {
    Arc::new(SchemaUnit {
        metadata: ExtensionMetadata {
            id: "kz-core",
            name: "kz-core",
            version: "1.0.0",
        },
        standing: StandingTag::Core,
        schemas: vec![],
        migrations: vec![Migration::new(
            1,
            "create",
            "CREATE TABLE kz_kept (id INT);\nCREATE TRIGGER kz_kept_trg BEFORE UPDATE ON kz_kept \
             FOR EACH ROW EXECUTE FUNCTION kz_f();",
        )],
        seeds: vec![],
        retirements: if retired {
            vec![SchemaDefinition::new(
                "kz_kept",
                "DROP TABLE IF EXISTS kz_kept;",
            )]
        } else {
            vec![]
        },
        cross_tables: vec![],
        purges: vec![],
    })
}

#[test]
fn a_retired_tables_triggers_leave_with_it() {
    let install_findings = |members: &[Arc<dyn SchemaMember>]| {
        described(
            migration_ownership_findings(only(members), &Recorded::NONE, &all_pending)
                .into_iter()
                .filter(|finding| finding.member().as_str() == "kz-install")
                .collect(),
        )
    };
    let declaring = || {
        member(
            "kz-install",
            StandingTag::Install,
            Some(
                "CREATE TABLE IF NOT EXISTS kz_kept (id INT);\nCREATE OR REPLACE TRIGGER kz_kept_trg \
                 BEFORE UPDATE ON kz_kept FOR EACH ROW EXECUTE FUNCTION kz_g();",
            ),
            None,
        )
    };
    assert!(
        install_findings(&[retiring_creator(true), declaring()]).is_empty(),
        "the retired table and its trigger have no owner"
    );
    assert_eq!(
        install_findings(&[retiring_creator(false), declaring()]),
        vec![
            "shared public.kz_kept".to_owned(),
            "object CREATE TRIGGER kz_kept_trg ON public.kz_kept".to_owned(),
        ],
        "negative control: without the retirement both stay the creator's"
    );
}
