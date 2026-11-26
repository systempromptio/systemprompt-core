//! Admission of re-seal declarations: a member declares only columns of a
//! table its own schema SQL creates and no other member creates, so it can
//! name neither a core table nor another member's table.

use systemprompt_extension::{
    AdmittedReseals, ResealAdmissionError, ResealClaim, ResealColumn, SchemaDefinition,
    SealedLayout,
};
use systemprompt_identifiers::ExtensionId;

fn schemas(tables: &[&str]) -> Vec<SchemaDefinition> {
    tables
        .iter()
        .map(|t| {
            SchemaDefinition::new(
                *t,
                format!("CREATE TABLE {t} (user_id TEXT, ciphertext BYTEA, nonce BYTEA)"),
            )
        })
        .collect()
}

const fn column(table: &'static str) -> ResealColumn {
    ResealColumn {
        table,
        keys: &["user_id"],
        ciphertext: "ciphertext",
        layout: SealedLayout::SplitBytea { nonce: "nonce" },
    }
}

fn admit(
    members: &[(&ExtensionId, &[SchemaDefinition], &[ResealColumn])],
) -> Result<AdmittedReseals, ResealAdmissionError> {
    let claims: Vec<ResealClaim<'_>> = members
        .iter()
        .map(|(owner, schemas, columns)| ResealClaim {
            owner,
            schemas,
            columns,
        })
        .collect();
    AdmittedReseals::admit(&claims)
}

#[test]
fn a_member_declares_a_column_of_its_own_table() {
    let install = ExtensionId::new("acme");
    let own = schemas(&["acme_credentials"]);
    let admitted = admit(&[(&install, &own, &[column("acme_credentials")])]).expect("own table");
    assert_eq!(
        admitted.of_member(&install),
        vec![&column("acme_credentials")]
    );
    assert!(
        admitted.of_member(&ExtensionId::new("other")).is_empty(),
        "a member that declared nothing has no column to re-seal"
    );
}

#[test]
fn a_core_table_is_refused() {
    let users = ExtensionId::new("users");
    let install = ExtensionId::new("acme");
    let core = schemas(&["users"]);
    let own = schemas(&["acme_credentials"]);
    let refused = admit(&[(&users, &core, &[]), (&install, &own, &[column("users")])]);
    assert!(
        matches!(&refused, Err(ResealAdmissionError::ForeignTable { member, table })
            if member == &install && table == "users"),
        "{refused:?}"
    );
}

#[test]
fn a_table_another_member_also_creates_is_refused() {
    let users = ExtensionId::new("users");
    let install = ExtensionId::new("acme");
    let core = schemas(&["users"]);
    let claims_users_too = schemas(&["acme_credentials", "users"]);
    let refused = admit(&[
        (&users, &core, &[]),
        (&install, &claims_users_too, &[column("users")]),
    ]);
    assert!(
        matches!(&refused, Err(ResealAdmissionError::SharedTable { member, other, table })
            if member == &install && other == &users && table == "users"),
        "{refused:?}"
    );
}

#[test]
fn another_install_members_table_is_refused() {
    let first = ExtensionId::new("acme");
    let second = ExtensionId::new("globex");
    let first_tables = schemas(&["acme_credentials"]);
    let second_tables = schemas(&["globex_credentials"]);
    let refused = admit(&[
        (&first, &first_tables, &[]),
        (&second, &second_tables, &[column("acme_credentials")]),
    ]);
    assert!(
        matches!(&refused, Err(ResealAdmissionError::ForeignTable { member, .. }) if member == &second),
        "{refused:?}"
    );
}

#[test]
fn identifiers_and_column_sets_are_judged() {
    let install = ExtensionId::new("acme");
    let own = schemas(&["acme_credentials"]);
    let quoted = ResealColumn {
        ciphertext: "ciphertext\"; DROP TABLE users; --",
        ..column("acme_credentials")
    };
    assert!(matches!(
        admit(&[(&install, &own, &[quoted])]),
        Err(ResealAdmissionError::Identifier { .. })
    ));
    let keyless = ResealColumn {
        keys: &[],
        ..column("acme_credentials")
    };
    assert!(matches!(
        admit(&[(&install, &own, &[keyless])]),
        Err(ResealAdmissionError::Columns { .. })
    ));
    let overlapping = ResealColumn {
        layout: SealedLayout::SplitBytea {
            nonce: "ciphertext",
        },
        ..column("acme_credentials")
    };
    assert!(matches!(
        admit(&[(&install, &own, &[overlapping])]),
        Err(ResealAdmissionError::Columns { .. })
    ));
}
