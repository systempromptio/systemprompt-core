use std::sync::Arc;

use sp_accounts::{Accounts, AccountsError};
use sp_client::ManifestKey;
use sp_protocol::ids::AccountId;
use sp_protocol::view::KeyOffer;
use sp_secure::MemorySealer;

use crate::support::{account, open, open_plain, store_path, write_store};

fn alpha(dir: &tempfile::TempDir) -> (Accounts, AccountId) {
    let path = store_path(dir);
    write_store(
        &path,
        &serde_json::json!({ "version": 1, "accounts": [account("a1", "Alpha")] }),
    );
    (open_plain(&path), AccountId::new("a1"))
}

fn is_nothing_to_trust(result: &Result<(), AccountsError>) -> bool {
    matches!(result, Err(AccountsError::NothingToTrust { .. }))
}

#[test]
fn the_first_key_is_offered_never_trusted() {
    let dir = tempfile::tempdir().unwrap();
    let (accounts, id) = alpha(&dir);
    let first = ManifestKey::from_raw([7u8; 32]);
    accounts.offer_key(&id, &first).unwrap();
    assert!(accounts.pinned_key(&id).unwrap().is_none());
    let offer = KeyOffer {
        pinned: None,
        offered: first.fingerprint().clone(),
    };
    assert_eq!(accounts.key_offer(&id).unwrap(), Some(offer));
    assert_eq!(accounts.snapshot().offers.len(), 1);
    let shown = accounts.get(&id).unwrap();
    assert_eq!(shown.fingerprint.as_ref(), Some(first.fingerprint()));
    assert!(shown.pinned.is_none());
    assert!(!shown.trusted);
}

#[test]
fn trust_pins_exactly_the_fingerprint_shown() {
    let dir = tempfile::tempdir().unwrap();
    let (accounts, id) = alpha(&dir);
    let first = ManifestKey::from_raw([7u8; 32]);
    let other = ManifestKey::from_raw([9u8; 32]);
    accounts.offer_key(&id, &first).unwrap();
    assert!(is_nothing_to_trust(
        &accounts.trust(&id, other.fingerprint())
    ));
    assert!(accounts.key_offer(&id).unwrap().is_some());
    accounts.trust(&id, first.fingerprint()).unwrap();
    assert_eq!(accounts.pinned_key(&id).unwrap(), Some(first.clone()));
    assert!(accounts.key_offer(&id).unwrap().is_none());
    assert!(accounts.get(&id).unwrap().trusted);
    assert_eq!(
        accounts.snapshot().offers,
        Vec::<sp_accounts::PendingKeyOffer>::new()
    );
    assert!(is_nothing_to_trust(
        &accounts.trust(&id, first.fingerprint())
    ));
    accounts.offer_key(&id, &first).unwrap();
    assert!(accounts.key_offer(&id).unwrap().is_none());
}

#[test]
fn a_rotated_key_blocks_until_trusted() {
    let dir = tempfile::tempdir().unwrap();
    let (accounts, id) = alpha(&dir);
    let first = ManifestKey::from_raw([7u8; 32]);
    let rotated = ManifestKey::from_raw([8u8; 32]);
    accounts.offer_key(&id, &first).unwrap();
    accounts.trust(&id, first.fingerprint()).unwrap();
    accounts.offer_key(&id, &rotated).unwrap();
    assert!(accounts.pinned_key(&id).unwrap().is_none());
    let offer = KeyOffer {
        pinned: Some(first.fingerprint().clone()),
        offered: rotated.fingerprint().clone(),
    };
    assert_eq!(accounts.key_offer(&id).unwrap(), Some(offer));
    accounts.offer_key(&id, &first).unwrap();
    assert!(accounts.key_offer(&id).unwrap().is_none());
    assert_eq!(accounts.pinned_key(&id).unwrap(), Some(first));
    accounts.offer_key(&id, &rotated).unwrap();
    accounts.trust(&id, rotated.fingerprint()).unwrap();
    assert_eq!(accounts.pinned_key(&id).unwrap(), Some(rotated));
}

#[test]
fn a_trusted_key_survives_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let (accounts, id) = alpha(&dir);
    let first = ManifestKey::from_raw([7u8; 32]);
    accounts.offer_key(&id, &first).unwrap();
    accounts.trust(&id, first.fingerprint()).unwrap();
    drop(accounts);
    let reopened = open(
        &store_path(&dir),
        Arc::new(MemorySealer::for_tests()),
        Arc::default(),
    );
    assert_eq!(reopened.pinned_key(&id).unwrap(), Some(first));
}

#[test]
fn key_calls_on_an_unknown_account_fail() {
    let dir = tempfile::tempdir().unwrap();
    let (accounts, _) = alpha(&dir);
    let unknown = AccountId::new("missing");
    let key = ManifestKey::from_raw([7u8; 32]);
    assert!(matches!(
        accounts.offer_key(&unknown, &key),
        Err(AccountsError::UnknownAccount { .. })
    ));
    assert!(matches!(
        accounts.pinned_key(&unknown),
        Err(AccountsError::UnknownAccount { .. })
    ));
}
