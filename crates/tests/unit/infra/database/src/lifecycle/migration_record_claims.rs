//! Ownership records judged against the code of the members present: a
//! record carrying install standing never outranks a core member that
//! creates the same table or object, a judged member's own record never
//! keeps an object its code no longer creates, so a retired table's trigger
//! records leave with it, and an install member sharing a core record's id
//! does not inherit it. A held-back member's and an absent member's records
//! keep their objects (K-SEC8-2, K-SEC8-4, K-SEC8-7).

use std::sync::Arc;

use systemprompt_database::lifecycle::{
    MigrationMembers, MigrationOwnershipError, ObjectKind, Recorded, RecordedObject, RecordedTable,
    migration_ownership_findings,
};
use systemprompt_database::{SchemaMember, SchemaUnit};
use systemprompt_extension::{ExtensionMetadata, Migration, SchemaDefinition, StandingTag};
use systemprompt_identifiers::ExtensionId;

use super::migration_ownership::{all_pending, only, unit};

pub(super) fn member(
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

pub(super) fn object(
    kind: ObjectKind,
    name: &str,
    owner: &str,
    standing: StandingTag,
) -> RecordedObject {
    RecordedObject {
        kind,
        name: name.to_owned(),
        owner: ExtensionId::new(owner),
        standing,
    }
}

pub(super) fn table(name: &str, owner: &str, standing: StandingTag) -> RecordedTable {
    RecordedTable {
        table: name.to_owned(),
        owner: ExtensionId::new(owner),
        standing,
    }
}

pub(super) fn described(
    members: MigrationMembers<'_>,
    tables: &[RecordedTable],
    objects: &[RecordedObject],
) -> Vec<String> {
    migration_ownership_findings(members, &Recorded { tables, objects }, &all_pending)
        .into_iter()
        .map(|finding| match finding {
            MigrationOwnershipError::ForeignObject {
                member,
                statement,
                object,
                owner,
                ..
            } => format!("{member}: object {statement} {object} owned by {owner}"),
            MigrationOwnershipError::ForeignTable {
                member,
                statement,
                table,
                owner,
                ..
            } => format!("{member}: table {statement} {table} owned by {owner}"),
            MigrationOwnershipError::SharedTable {
                member,
                table,
                owner,
                ..
            } => format!("{member}: shared {table} owned by {owner}"),
            other => format!("{other}"),
        })
        .collect()
}

const FN: &str = "CREATE OR REPLACE FUNCTION kr_f() RETURNS INT LANGUAGE sql AS $$ SELECT 1 $$;";

#[test]
fn a_stale_install_record_never_outranks_a_core_object() {
    let records = [object(
        ObjectKind::Object,
        "public.kr_f",
        "kr-install",
        StandingTag::Install,
    )];
    let install = member("kr-install", StandingTag::Install, None, Some(FN));
    let core = member("kr-core", StandingTag::Core, Some(FN), None);
    assert_eq!(
        described(only(&[core, install.clone()]), &[], &records),
        vec!["kr-install: object CREATE public.kr_f owned by kr-core".to_owned()]
    );
    assert!(
        described(only(&[install]), &[], &records).is_empty(),
        "negative control: without a core creator the install's record holds"
    );
}

#[test]
fn a_table_record_from_the_old_ranking_never_outranks_a_core_migration() {
    let records = [table("public.kr_t", "kr-install", StandingTag::Install)];
    let install = member(
        "kr-install",
        StandingTag::Install,
        None,
        Some("ALTER TABLE kr_t ADD COLUMN x INT;"),
    );
    let core = member(
        "kr-core",
        StandingTag::Core,
        None,
        Some("CREATE TABLE kr_t (id INT);"),
    );
    assert_eq!(
        described(only(&[core, install.clone()]), &records, &[]),
        vec!["kr-install: table ALTER TABLE public.kr_t owned by kr-core".to_owned()]
    );
    assert!(
        described(only(&[install]), &records, &[]).is_empty(),
        "negative control: without a core creator the install's record holds"
    );
}

#[test]
fn a_core_record_still_outranks_an_install_creator() {
    let records = [object(
        ObjectKind::Object,
        "public.kr_f",
        "kr-gone",
        StandingTag::Core,
    )];
    let install = member("kr-install", StandingTag::Install, None, Some(FN));
    assert_eq!(
        described(only(std::slice::from_ref(&install)), &[], &records),
        vec!["kr-install: object CREATE public.kr_f owned by kr-gone".to_owned()]
    );
    assert!(
        described(only(&[install]), &[], &[]).is_empty(),
        "negative control: without the record the install creates its own"
    );
}

#[test]
fn a_judged_members_own_record_releases_an_object_its_code_no_longer_creates() {
    let records = [object(
        ObjectKind::Object,
        "public.kr_dropped",
        "kr-owner",
        StandingTag::Install,
    )];
    let owner = member(
        "kr-owner",
        StandingTag::Install,
        Some("CREATE TABLE kr_o (id INT);"),
        None,
    );
    let peer = member(
        "kr-peer",
        StandingTag::Install,
        None,
        Some("DROP FUNCTION kr_dropped();"),
    );
    let composition = [owner.clone(), peer.clone()];
    assert!(
        described(only(&composition), &[], &records).is_empty(),
        "the owner's code no longer creates it, so its record is no claim"
    );
    assert_eq!(
        described(
            MigrationMembers::new(&composition, std::slice::from_ref(&owner)),
            &[],
            &records
        ),
        vec!["kr-peer: object DROP public.kr_dropped owned by kr-owner".to_owned()],
        "negative control: a held-back owner keeps its record"
    );
    assert_eq!(
        described(only(&[peer]), &[], &records),
        vec!["kr-peer: object DROP public.kr_dropped owned by kr-owner".to_owned()],
        "negative control: an absent owner keeps its record"
    );
}

fn retiring(retired: bool) -> Arc<dyn SchemaMember> {
    Arc::new(SchemaUnit {
        metadata: ExtensionMetadata {
            id: "kr-core",
            name: "kr-core",
            version: "1.0.0",
        },
        standing: StandingTag::Core,
        schemas: vec![],
        migrations: vec![Migration::new(
            1,
            "create",
            "CREATE TABLE kr_kept (id INT);\nCREATE TRIGGER kr_kept_trg BEFORE UPDATE ON kr_kept \
             FOR EACH ROW EXECUTE FUNCTION kr_g();",
        )],
        seeds: vec![],
        retirements: if retired {
            vec![SchemaDefinition::new(
                "kr_kept",
                "DROP TABLE IF EXISTS kr_kept;",
            )]
        } else {
            vec![]
        },
        cross_tables: vec![],
        purges: vec![],
        reassignments: vec![],
    })
}

#[test]
fn a_retired_tables_trigger_record_is_released() {
    let records = [object(
        ObjectKind::Trigger,
        "kr_kept_trg ON public.kr_kept",
        "kr-core",
        StandingTag::Core,
    )];
    let tables = [table("public.kr_kept", "kr-core", StandingTag::Core)];
    let dropping = || {
        member(
            "kr-install",
            StandingTag::Install,
            None,
            Some("DROP TRIGGER kr_kept_trg ON kr_kept;"),
        )
    };
    assert!(
        described(only(&[retiring(true), dropping()]), &tables, &records).is_empty(),
        "the retired table's trigger record is no claim"
    );
    assert_eq!(
        described(only(&[retiring(false), dropping()]), &tables, &records),
        vec![
            "kr-install: object DROP TRIGGER kr_kept_trg ON public.kr_kept owned by kr-core"
                .to_owned()
        ],
        "negative control: without the retirement the record and the code keep it"
    );
}

#[test]
fn an_install_with_a_core_records_id_does_not_inherit_it() {
    let records = [object(
        ObjectKind::Object,
        "public.kr_f",
        "kr-core",
        StandingTag::Core,
    )];
    let tables = [table("public.kr_t", "kr-core", StandingTag::Core)];
    let sql = "DROP FUNCTION kr_f();\nALTER TABLE kr_t ADD COLUMN x INT;";
    assert_eq!(
        described(
            only(&[member("kr-core", StandingTag::Install, None, Some(sql))]),
            &tables,
            &records
        ),
        vec![
            "kr-core: object DROP public.kr_f owned by kr-core".to_owned(),
            "kr-core: table ALTER TABLE public.kr_t owned by kr-core".to_owned(),
        ]
    );
    assert!(
        described(
            only(&[member("kr-core", StandingTag::Core, None, Some(sql))]),
            &tables,
            &records
        )
        .is_empty(),
        "negative control: the core member itself owns them"
    );
}
