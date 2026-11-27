//! A record whose owner is held back or outside the composition outranks
//! every member's code, a core member's included: a core create of its table
//! or object is refused and the record stands. Only a judged owner's record
//! yields to a core create, and that owner's own create is then refused. A
//! member with a tombstoned migration keeps its object records. An install
//! schema that does not parse, and an install view, materialized view or
//! rule reading a kernel ledger, are refused.

use std::sync::Arc;

use systemprompt_database::SchemaMember;
use systemprompt_database::lifecycle::{
    MigrationMembers, MigrationOwnershipError, ObjectKind, Recorded, migration_ownership_findings,
};
use systemprompt_extension::{Migration, StandingTag};

use super::migration_ownership::{all_pending, only, unit};
use super::migration_record_claims::{described, member, object, table};

const TABLE: &str = "CREATE TABLE IF NOT EXISTS ka_t (id INT PRIMARY KEY);";
const FN: &str = "CREATE OR REPLACE FUNCTION ka_f() RETURNS INT LANGUAGE sql AS $$ SELECT 1 $$;";

fn core(schema: &'static str) -> Arc<dyn SchemaMember> {
    member("ka-core", StandingTag::Core, Some(schema), None)
}

#[test]
fn a_core_create_of_an_absent_or_held_back_owners_table_is_refused() {
    let records = [table("public.ka_t", "ka-owner", StandingTag::Install)];
    assert_eq!(
        described(only(&[core(TABLE)]), &records, &[]),
        vec!["ka-core: shared public.ka_t owned by ka-owner".to_owned()],
        "an absent owner's record outranks the core create"
    );
    let owner = member(
        "ka-owner",
        StandingTag::Install,
        Some("CREATE TABLE IF NOT EXISTS ka_t (id INT PRIMARY KEY);"),
        None,
    );
    let composition = [core(TABLE), owner.clone()];
    assert_eq!(
        described(
            MigrationMembers::new(&composition, std::slice::from_ref(&owner)),
            &records,
            &[]
        ),
        vec!["ka-core: shared public.ka_t owned by ka-owner".to_owned()],
        "a held-back owner's record outranks the core create"
    );
    assert_eq!(
        described(only(&composition), &records, &[]),
        vec!["ka-owner: shared public.ka_t owned by ka-core".to_owned()],
        "control: a judged owner's record yields, and the owner's own create is refused"
    );
    let moved_on = member(
        "ka-owner",
        StandingTag::Install,
        Some("CREATE TABLE IF NOT EXISTS ka_other (id INT PRIMARY KEY);"),
        None,
    );
    assert!(
        described(only(&[core(TABLE), moved_on]), &records, &[]).is_empty(),
        "control: a judged owner that no longer creates the table leaves it to core"
    );
}

#[test]
fn a_core_create_of_an_absent_or_held_back_owners_object_is_refused() {
    let records = [object(
        ObjectKind::Object,
        "public.ka_f",
        "ka-owner",
        StandingTag::Install,
    )];
    let refused = vec!["ka-core: object CREATE public.ka_f owned by ka-owner".to_owned()];
    assert_eq!(
        described(only(&[core(FN)]), &[], &records),
        refused,
        "a core declarative schema's create is refused"
    );
    let migrating = member("ka-core", StandingTag::Core, None, Some(FN));
    assert_eq!(
        described(only(&[migrating]), &[], &records),
        refused,
        "a core pending migration's create is refused"
    );
    let owner = member("ka-owner", StandingTag::Install, Some(FN), None);
    let composition = [core(FN), owner.clone()];
    assert_eq!(
        described(
            MigrationMembers::new(&composition, std::slice::from_ref(&owner)),
            &[],
            &records
        ),
        refused,
        "a held-back owner's record outranks the core create"
    );
    assert_eq!(
        described(only(&composition), &[], &records),
        vec!["ka-owner: object CREATE public.ka_f owned by ka-core".to_owned()],
        "control: a judged owner's record yields to the core create"
    );
    assert!(
        described(only(&[core(FN)]), &[], &[]).is_empty(),
        "control: without the record the core member creates its own"
    );
}

fn tombstoned(tombstone: bool) -> Arc<dyn SchemaMember> {
    let mut migrations = vec![Migration::new(2, "table", "CREATE TABLE ka_o (id INT);")];
    if tombstone {
        migrations.insert(0, Migration::tombstone(1, "made_ka_f"));
    }
    unit("ka-owner", StandingTag::Install, None, migrations, vec![])
}

#[test]
fn a_member_with_a_tombstoned_migration_keeps_its_object_records() {
    let records = [object(
        ObjectKind::Object,
        "public.ka_f",
        "ka-owner",
        StandingTag::Install,
    )];
    let peer = member(
        "ka-peer",
        StandingTag::Install,
        None,
        Some("DROP FUNCTION ka_f();"),
    );
    assert_eq!(
        described(only(&[tombstoned(true), peer.clone()]), &[], &records),
        vec!["ka-peer: object DROP public.ka_f owned by ka-owner".to_owned()],
        "a tombstone carries no SQL, so the record stands"
    );
    assert!(
        described(only(&[tombstoned(false), peer]), &[], &records).is_empty(),
        "control: without a tombstone the owner's code decides"
    );
}

fn first_finding(schema: &'static str) -> Option<String> {
    let install = unit(
        "ka-install",
        StandingTag::Install,
        Some(schema),
        vec![],
        vec![],
    );
    migration_ownership_findings(only(&[install]), &Recorded::NONE, &all_pending)
        .into_iter()
        .next()
        .map(|finding| match finding {
            MigrationOwnershipError::Parse {
                member, migration, ..
            } => {
                format!("{member}: parse {migration}")
            },
            MigrationOwnershipError::ForeignTable {
                member,
                statement,
                table,
                owner,
                ..
            } => format!("{member}: table {statement} {table} owned by {owner}"),
            other => format!("{other}"),
        })
}

#[test]
fn an_install_schema_that_does_not_parse_is_refused() {
    assert_eq!(
        first_finding("CREATE TABLE ka_t (id INT PRIMARY KEY;"),
        Some("ka-install: parse schema".to_owned())
    );
    assert_eq!(first_finding(TABLE), None, "control: a parseable schema");
}

#[test]
fn an_install_view_matview_or_rule_reading_a_kernel_ledger_is_refused() {
    for (sql, statement) in [
        (
            "CREATE TABLE ka_t (id INT);
CREATE VIEW ka_v AS SELECT * FROM schema_object_owners;",
            "CREATE",
        ),
        (
            "CREATE TABLE ka_t (id INT);
CREATE VIEW ka_v AS SELECT id FROM ka_t              WHERE EXISTS (SELECT 1 FROM public.schema_object_owners);",
            "CREATE",
        ),
        (
            "CREATE TABLE ka_t (id INT);
CREATE MATERIALIZED VIEW ka_mv AS              SELECT * FROM schema_object_owners;",
            "CREATE",
        ),
        (
            "CREATE TABLE ka_t (id INT);
CREATE RULE ka_r AS ON INSERT TO ka_t              DO ALSO SELECT * FROM schema_object_owners;",
            "CREATE RULE",
        ),
    ] {
        let sql: &'static str = Box::leak(sql.to_owned().into_boxed_str());
        assert_eq!(
            first_finding(sql),
            Some(format!(
                "ka-install: table {statement} public.schema_object_owners owned by database"
            )),
            "{sql}"
        );
    }
    assert_eq!(
        first_finding(
            "CREATE TABLE ka_t (id INT);
CREATE VIEW ka_v AS SELECT id FROM ka_t;"
        ),
        None,
        "control: a view over the member's own table is admitted"
    );
}
