use std::path::Path;
use std::sync::Arc;

use sp_accounts::{Accounts, AccountsError, set_aside_corrupt_store};
use sp_protocol::ManifestVersion;
use sp_protocol::ids::{AccountId, InstallId, UserId};
use sp_protocol::view::AccountStatus;
use sp_secure::{MemorySealer, SealedString, Sealer};

use crate::support::{account, config, open, open_plain, store_path, write_store};

const VERSION: &str = "2026-11-01T09:12:31Z-000001a19661c117";

fn open_error(path: &Path, bytes: &[u8]) -> AccountsError {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
    let opened = Accounts::open(config(
        path,
        Arc::new(MemorySealer::for_tests()),
        Arc::default(),
    ));
    let error = opened.unwrap_err();
    assert_eq!(std::fs::read(path).unwrap(), bytes);
    error
}

fn sealed(sealer: &dyn Sealer, secret: &str) -> serde_json::Value {
    serde_json::to_value(SealedString::seal(sealer, secret).unwrap()).unwrap()
}

#[test]
fn the_store_round_trips_and_keeps_unknown_fields() {
    let dir = tempfile::tempdir().unwrap();
    let path = store_path(&dir);
    let sealer: Arc<dyn Sealer> = Arc::new(MemorySealer::for_tests());
    let pat = sealed(&*sealer, "sp-live-secret");
    let mut alpha = account("a1", "Alpha");
    alpha["instance_key"] = serde_json::json!("alpha");
    alpha["pat"] = pat.clone();
    alpha["user_id"] = serde_json::json!("user-1");
    alpha["applied_version"] = serde_json::json!(VERSION);
    alpha["active"] = serde_json::json!(true);
    alpha["future_field"] = serde_json::json!({ "kept": true });
    let store = serde_json::json!({
        "version": 1,
        "install_id": "install-stored",
        "accounts": [alpha],
        "future_top": 7,
    });
    write_store(&path, &store);

    let accounts = open(&path, Arc::clone(&sealer), Arc::default());
    let id = AccountId::new("a1");
    accounts
        .set_outbox(&id, Some("outbox-1.json".to_owned()))
        .unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains("sp-live-secret"));
    let back: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(back["future_top"], serde_json::json!(7));
    assert_eq!(back["install_id"], "install-stored");
    assert_eq!(
        back["accounts"][0]["future_field"],
        serde_json::json!({ "kept": true })
    );
    assert_eq!(back["accounts"][0]["pat"], pat);
    assert_eq!(back["accounts"][0]["outbox"], "outbox-1.json");
    assert!(text.ends_with('\n'));

    assert_eq!(accounts.install_id(), InstallId::new("install-stored"));
    assert_eq!(accounts.user_id(&id).unwrap(), Some(UserId::new("user-1")));
    let applied = accounts.last_applied_version(&id).unwrap();
    assert_eq!(applied.as_ref().map(ManifestVersion::as_str), Some(VERSION));
    let shown = accounts.get(&id).unwrap();
    assert!(shown.active);
    assert_eq!(shown.status, AccountStatus::SignedIn);
}

#[test]
fn a_corrupt_or_newer_store_is_reported_and_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let path = store_path(&dir);
    assert!(matches!(
        open_error(&path, b"{not json"),
        AccountsError::StoreCorrupt {
            category: "syntax",
            ..
        }
    ));
    assert!(matches!(
        open_error(&path, br#"{"accounts":[]}"#),
        AccountsError::StoreCorrupt { .. }
    ));
    assert!(matches!(
        open_error(&path, br#"{"version":99}"#),
        AccountsError::StoreVersion { found: 99, .. }
    ));
    assert!(matches!(
        open_error(&path, br#"{"version":0}"#),
        AccountsError::StoreVersion { found: 0, .. }
    ));
    assert!(matches!(
        open_error(&path, br#"{"version":1,"accounts":[{"id":5}]}"#),
        AccountsError::StoreCorrupt {
            category: "data",
            ..
        }
    ));
}

#[test]
fn a_missing_store_starts_empty_and_a_corrupt_one_can_be_set_aside() {
    let dir = tempfile::tempdir().unwrap();
    let path = store_path(&dir);
    let fresh = open_plain(&path);
    assert_eq!(fresh.list(), Vec::<sp_protocol::view::AccountView>::new());
    assert!(fresh.snapshot().needs_sign_in());
    assert!(path.exists());
    drop(fresh);

    std::fs::write(&path, b"garbage").unwrap();
    assert!(matches!(
        open_error(&path, b"garbage"),
        AccountsError::StoreCorrupt { .. }
    ));
    let aside = set_aside_corrupt_store(&path).unwrap();
    assert!(!path.exists());
    assert_eq!(std::fs::read(&aside).unwrap(), b"garbage");
    assert!(
        aside
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("accounts.json.corrupt-")
    );
    assert_eq!(
        open_plain(&path).list(),
        Vec::<sp_protocol::view::AccountView>::new()
    );
}

#[test]
fn opening_keeps_one_active_signed_in_account_and_unique_keys() {
    let dir = tempfile::tempdir().unwrap();
    let path = store_path(&dir);
    let sealer: Arc<dyn Sealer> = Arc::new(MemorySealer::for_tests());
    let mut first = account("a1", "Alpha");
    first["instance_key"] = serde_json::json!("alpha");
    first["active"] = serde_json::json!(true);
    let mut second = account("a2", "Alpha");
    second["pat"] = sealed(&*sealer, "p");
    second["active"] = serde_json::json!(true);
    let mut third = account("a3", "Beta");
    third["instance_key"] = serde_json::json!("beta");
    third["pat"] = sealed(&*sealer, "q");
    third["active"] = serde_json::json!(true);
    write_store(
        &path,
        &serde_json::json!({ "version": 1, "accounts": [first, second, third] }),
    );

    let accounts = open(&path, Arc::clone(&sealer), Arc::default());
    assert_eq!(accounts.active_id(), Some(AccountId::new("a2")));
    let active: Vec<String> = accounts
        .list()
        .into_iter()
        .filter(|shown| shown.active)
        .map(|shown| shown.id.into_inner())
        .collect();
    assert_eq!(active, ["a2"]);
    let keys: Vec<String> = ["a1", "a2", "a3"]
        .iter()
        .map(|id| {
            accounts
                .instance_key(&AccountId::new(*id))
                .unwrap()
                .into_inner()
        })
        .collect();
    assert_eq!(keys, ["alpha", "alpha-2", "beta"]);
    drop(accounts);

    let normalized = std::fs::read(&path).unwrap();
    drop(open(&path, sealer, Arc::default()));
    assert_eq!(std::fs::read(&path).unwrap(), normalized);
}
