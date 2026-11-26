//! `admin secrets reseal` over a declared column: a dry run writes nothing;
//! an apply re-seals pre-0.88 ciphertext (ChaCha20-Poly1305 directly under the
//! master key) under the member's cipher, leaves a value that opens under
//! neither scheme untouched and reports it; a second apply is a no-op
//! recognised by the envelope; a re-sealed value opens with the member's
//! cipher and no longer under the raw scheme. Core's own columns (the `TEXT`
//! hex layout) move the same way, and a value core seals now can neither be
//! opened under the raw scheme nor be re-sealed into an install's cipher.

use std::sync::Arc;

use chacha20poly1305::aead::Aead;
use chacha20poly1305::{ChaCha20Poly1305, KeyInit, Nonce};
use sqlx::{PgPool, Row};
use systemprompt_cli::commands::admin::secrets::reseal::{
    ColumnReport, ResealCiphers, RowState, check_recorded_owner, reseal_column,
};
use systemprompt_database::admin::{ResealStore, SealedRow};
use systemprompt_extension::{ResealColumn, SealedLayout};
use systemprompt_identifiers::ExtensionId;
use systemprompt_security::at_rest::{AtRestCipher, AtRestCipherError, pre_088};
use systemprompt_security::conferred::MemberCiphers;
use systemprompt_test_fixtures::{ScratchSchema, master_key_of};
use systemprompt_traits::{ConferralIssuer, MEMBER_CIPHER_ENVELOPE, MemberCipher, MemberGrant};

const MASTER: [u8; 32] = [7; 32];

const COLUMN: ResealColumn = ResealColumn {
    table: "reseal_credentials",
    keys: &["user_id", "provider"],
    ciphertext: "ciphertext",
    layout: SealedLayout::SplitBytea { nonce: "nonce" },
};

const CORE_COLUMN: ResealColumn = ResealColumn {
    table: "reseal_core_keys",
    keys: &["user_id"],
    ciphertext: "ciphertext",
    layout: SealedLayout::HexText,
};

fn raw_seal(plaintext: &[u8]) -> ([u8; 12], Vec<u8>) {
    let nonce: [u8; 12] = rand::random();
    let sealed = ChaCha20Poly1305::new(&MASTER.into())
        .encrypt(&Nonce::from(nonce), plaintext)
        .expect("raw seal");
    (nonce, sealed)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

async fn seeded(pool: &PgPool) {
    sqlx::query(
        "CREATE TABLE reseal_credentials (user_id TEXT NOT NULL, provider TEXT NOT NULL, \
         ciphertext BYTEA NOT NULL, nonce BYTEA NOT NULL, PRIMARY KEY (user_id, provider))",
    )
    .execute(pool)
    .await
    .expect("create the declared table");
    for (user, provider, secret) in [
        ("u1", "github", "gh-token"),
        ("u1", "slack", "sl-token"),
        ("u2", "github", "other-token"),
    ] {
        let (nonce, sealed) = raw_seal(secret.as_bytes());
        sqlx::query("INSERT INTO reseal_credentials VALUES ($1, $2, $3, $4)")
            .bind(user)
            .bind(provider)
            .bind(sealed)
            .bind(nonce.to_vec())
            .execute(pool)
            .await
            .expect("seed a raw-sealed row");
    }
    sqlx::query(
        "INSERT INTO reseal_credentials VALUES ('u3', 'broken', '\\x00ff', \
         '\\x000000000000000000000000')",
    )
    .execute(pool)
    .await
    .expect("seed an undecodable row");
}

async fn stored(pool: &PgPool) -> Vec<(String, Vec<u8>, Vec<u8>)> {
    sqlx::query(
        "SELECT user_id || '/' || provider AS k, ciphertext, nonce FROM reseal_credentials ORDER \
         BY 1",
    )
    .fetch_all(pool)
    .await
    .expect("read back")
    .iter()
    .map(|row| (row.get("k"), row.get("ciphertext"), row.get("nonce")))
    .collect()
}

fn report(column: &ResealColumn, counts: [u64; 4]) -> ColumnReport {
    ColumnReport {
        table: column.table.to_owned(),
        column: column.ciphertext.to_owned(),
        total: counts[0],
        member_sealed: counts[1],
        resealed: counts[2],
        undecodable: counts[3],
        unwritten: 0,
    }
}

fn member_cipher(member: &str) -> Arc<dyn MemberCipher> {
    MemberCiphers::from_master_key(&master_key_of(MASTER))
        .expect("issuer")
        .issue(&MemberGrant {
            member: ExtensionId::new(member),
            token_audiences: Arc::from([]),
        })
}

#[tokio::test]
async fn a_member_column_is_resealed_once_and_reported() {
    let schema = ScratchSchema::create("reseal").await;
    let pool = schema.pool(2).await;
    seeded(&pool).await;
    let store = ResealStore::new(Arc::new(pool.clone()));
    let member = ExtensionId::new("acme");
    let ciphers = ResealCiphers::new(master_key_of(MASTER), &member).expect("ciphers");

    let before = stored(&pool).await;
    let dry = reseal_column(&store, &COLUMN, &ciphers, false)
        .await
        .expect("dry run");
    assert_eq!(dry, report(&COLUMN, [4, 0, 3, 1]));
    assert_eq!(stored(&pool).await, before, "a dry run writes nothing");

    let applied = reseal_column(&store, &COLUMN, &ciphers, true)
        .await
        .expect("apply");
    assert_eq!(applied, report(&COLUMN, [4, 0, 3, 1]));
    let after = stored(&pool).await;
    let cipher = member_cipher("acme");
    let plaintexts: [&[u8]; 3] = [b"gh-token", b"sl-token", b"other-token"];
    for ((key, ciphertext, nonce), plaintext) in after.iter().take(3).zip(plaintexts) {
        let sealed = std::str::from_utf8(ciphertext).expect("the envelope is text");
        assert!(sealed.starts_with(MEMBER_CIPHER_ENVELOPE), "{key}");
        assert_eq!(
            cipher.open(sealed).expect("member opens"),
            plaintext,
            "{key}"
        );
        assert!(
            pre_088::open(&master_key_of(MASTER), nonce, ciphertext).is_err(),
            "{key} no longer opens under the raw scheme"
        );
    }
    assert_eq!(before[3], after[3], "an undecodable row is left as it is");

    let again = reseal_column(&store, &COLUMN, &ciphers, true)
        .await
        .expect("second apply");
    assert_eq!(
        again,
        report(&COLUMN, [4, 3, 0, 1]),
        "a second apply is a no-op"
    );
    assert_eq!(stored(&pool).await, after);
}

#[tokio::test]
async fn a_core_text_column_is_resealed_under_the_core_members_cipher() {
    let schema = ScratchSchema::create("resealcore").await;
    let pool = schema.pool(2).await;
    sqlx::query(
        "CREATE TABLE reseal_core_keys (user_id TEXT PRIMARY KEY, ciphertext TEXT NOT NULL)",
    )
    .execute(&pool)
    .await
    .expect("create the core-shaped table");
    let (nonce, sealed) = raw_seal(b"personal-key");
    let raw_hex = format!("{}{}", hex(&nonce), hex(&sealed));
    sqlx::query("INSERT INTO reseal_core_keys VALUES ('u1', $1)")
        .bind(&raw_hex)
        .execute(&pool)
        .await
        .expect("seed a pre-0.88 core value");
    let store = ResealStore::new(Arc::new(pool.clone()));
    let ai = ExtensionId::new("ai");
    let ciphers = ResealCiphers::new(master_key_of(MASTER), &ai).expect("ciphers");
    let applied = reseal_column(&store, &CORE_COLUMN, &ciphers, true)
        .await
        .expect("apply");
    assert_eq!(applied, report(&CORE_COLUMN, [1, 0, 1, 0]));
    let value: String = sqlx::query_scalar("SELECT ciphertext FROM reseal_core_keys")
        .fetch_one(&pool)
        .await
        .expect("read back");
    let core = AtRestCipher::for_member(&master_key_of(MASTER), &ai).expect("core cipher");
    assert_eq!(
        core.open(&value).expect("the ai member opens it"),
        "personal-key"
    );
    assert!(
        matches!(core.open(&raw_hex), Err(AtRestCipherError::Unresealed)),
        "a runtime reader refuses a pre-0.88 value, naming the reseal command"
    );
    let again = reseal_column(&store, &CORE_COLUMN, &ciphers, true)
        .await
        .expect("again");
    assert_eq!(again, report(&CORE_COLUMN, [1, 1, 0, 0]));
}

#[test]
fn a_value_core_seals_now_is_no_reseal_source() {
    let ai = AtRestCipher::for_member(&master_key_of(MASTER), &ExtensionId::new("ai"))
        .expect("core cipher");
    let sealed = ai.seal("provider key").expect("core seals");
    let bare = sealed
        .strip_prefix(MEMBER_CIPHER_ENVELOPE)
        .expect("enveloped");
    assert!(
        pre_088::open_hex(&master_key_of(MASTER), bare).is_err(),
        "a core value sealed now does not open under the raw scheme"
    );
    let install =
        ResealCiphers::new(master_key_of(MASTER), &ExtensionId::new("acme")).expect("ciphers");
    for row in [
        SealedRow {
            keys: vec![Some("k".to_owned())],
            ciphertext: sealed.clone().into_bytes(),
            nonce: Vec::new(),
        },
        SealedRow {
            keys: vec![Some("k".to_owned())],
            ciphertext: bare.as_bytes().to_vec(),
            nonce: Vec::new(),
        },
    ] {
        for layout in [
            SealedLayout::HexText,
            SealedLayout::SplitBytea { nonce: "nonce" },
        ] {
            assert_eq!(
                install.judge(layout, &row),
                RowState::Undecodable,
                "a planted core value is never re-sealed into an install's cipher"
            );
        }
    }
    assert!(
        member_cipher("acme").open(&sealed).is_err(),
        "an install's conferred cipher does not open a core member's value"
    );
}

#[test]
fn another_members_cipher_does_not_recognise_the_envelope() {
    let acme =
        ResealCiphers::new(master_key_of(MASTER), &ExtensionId::new("acme")).expect("ciphers");
    let globex =
        ResealCiphers::new(master_key_of(MASTER), &ExtensionId::new("globex")).expect("ciphers");
    let sealed = member_cipher("acme").seal(b"secret").expect("seal");
    let row = SealedRow {
        keys: vec![Some("k".to_owned())],
        ciphertext: sealed.into_bytes(),
        nonce: vec![0; 12],
    };
    assert_eq!(acme.judge(COLUMN.layout, &row), RowState::MemberSealed);
    assert_eq!(
        globex.judge(COLUMN.layout, &row),
        RowState::Undecodable,
        "an envelope that does not open under this member's cipher is never re-sealed"
    );
}

#[tokio::test]
async fn a_table_not_recorded_to_the_member_is_refused() {
    let schema = ScratchSchema::create("resealown").await;
    let pool = schema.pool(2).await;
    sqlx::query(
        "CREATE TABLE schema_table_owners (table_name TEXT PRIMARY KEY, extension_id TEXT NOT \
         NULL, standing TEXT NOT NULL)",
    )
    .execute(&pool)
    .await
    .expect("an owners table in the scratch schema");
    sqlx::query(
        "INSERT INTO schema_table_owners VALUES ('reseal_credentials', 'acme', 'install'), \
         ('reseal_core_keys', 'ai', 'core')",
    )
    .execute(&pool)
    .await
    .expect("record owners");
    let store = ResealStore::new(Arc::new(pool));
    check_recorded_owner(&store, &COLUMN, &ExtensionId::new("acme"))
        .await
        .expect("the recorded owner reseals");
    for (column, member) in [
        (&COLUMN, "globex"),
        (&CORE_COLUMN, "acme"),
        (
            &ResealColumn {
                table: "unrecorded_table",
                ..COLUMN
            },
            "acme",
        ),
    ] {
        assert!(
            check_recorded_owner(&store, column, &ExtensionId::new(member))
                .await
                .is_err(),
            "{} is not {member}'s",
            column.table
        );
    }
}

#[tokio::test]
async fn a_row_with_a_null_key_is_resealed_by_its_key_and_its_ciphertext() {
    let schema = ScratchSchema::create("resealnull").await;
    let pool = schema.pool(2).await;
    sqlx::query(
        "CREATE TABLE reseal_credentials (user_id TEXT, provider TEXT NOT NULL, ciphertext BYTEA \
         NOT NULL, nonce BYTEA NOT NULL)",
    )
    .execute(&pool)
    .await
    .expect("create a table whose key column is nullable");
    for user in [Some("u1"), None] {
        let (nonce, sealed) = raw_seal(b"token");
        sqlx::query("INSERT INTO reseal_credentials VALUES ($1, 'github', $2, $3)")
            .bind(user)
            .bind(sealed)
            .bind(nonce.to_vec())
            .execute(&pool)
            .await
            .expect("seed");
    }
    let store = ResealStore::new(Arc::new(pool.clone()));
    let ciphers =
        ResealCiphers::new(master_key_of(MASTER), &ExtensionId::new("acme")).expect("ciphers");
    let applied = reseal_column(&store, &COLUMN, &ciphers, true)
        .await
        .expect("apply");
    assert_eq!((applied.total, applied.resealed), (2, 2));
    assert_eq!(
        applied.undecided(),
        0,
        "a NULL key column leaves the operator nothing to do"
    );
    let null_keyed: Vec<u8> =
        sqlx::query_scalar("SELECT ciphertext FROM reseal_credentials WHERE user_id IS NULL")
            .fetch_one(&pool)
            .await
            .expect("read back");
    assert!(
        null_keyed.starts_with(MEMBER_CIPHER_ENVELOPE.as_bytes()),
        "the NULL-keyed row is re-sealed"
    );
    let opened = member_cipher("acme")
        .open(std::str::from_utf8(&null_keyed).expect("an envelope is text"))
        .expect("the re-sealed value opens under the member cipher");
    assert_eq!(opened, b"token");
    let again = reseal_column(&store, &COLUMN, &ciphers, true)
        .await
        .expect("again");
    assert_eq!(
        (again.member_sealed, again.resealed, again.undecided()),
        (2, 0, 0)
    );
}

#[tokio::test]
async fn a_null_keyed_row_whose_nonce_changed_is_not_written() {
    let schema = ScratchSchema::create("resealnonce").await;
    let pool = schema.pool(2).await;
    sqlx::query(
        "CREATE TABLE reseal_credentials (user_id TEXT, provider TEXT NOT NULL, ciphertext BYTEA \
         NOT NULL, nonce BYTEA NOT NULL)",
    )
    .execute(&pool)
    .await
    .expect("create");
    let (nonce, sealed) = raw_seal(b"token");
    sqlx::query("INSERT INTO reseal_credentials VALUES (NULL, 'github', $1, $2)")
        .bind(&sealed)
        .bind(nonce.to_vec())
        .execute(&pool)
        .await
        .expect("seed");
    let store = ResealStore::new(Arc::new(pool.clone()));
    let target = systemprompt_database::admin::ResealTable::new(
        COLUMN.table,
        COLUMN.keys,
        COLUMN.ciphertext,
        Some("nonce"),
    )
    .expect("target");
    let rows = store.page(&target, None, 10).await.expect("page");
    sqlx::query("UPDATE reseal_credentials SET nonce = '\\x010101010101010101010101'")
        .execute(&pool)
        .await
        .expect("a concurrent writer changes the nonce");
    let written = store
        .replace(
            &target,
            &[systemprompt_database::admin::Replacement {
                row: &rows[0],
                ciphertext: b"replacement".to_vec(),
            }],
        )
        .await
        .expect("replace");
    assert_eq!(written, 0, "a row whose nonce changed is never overwritten");
}
