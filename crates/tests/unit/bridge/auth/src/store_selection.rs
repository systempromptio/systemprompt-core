use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use keyring_core::api::{CredentialPersistence, CredentialStoreApi};
use keyring_core::{CredentialStore, Entry, Error};
use systemprompt_bridge::auth::setup::{self, SetupError};
use systemprompt_bridge::config::gateways::credential::{self, PatStore};
use tempfile::TempDir;

const GOOD: &str = "sp-live-testprefix.secretsecretsecretsecretsecret012345";

#[derive(Debug)]
struct RefusingStore;

impl CredentialStoreApi for RefusingStore {
    fn vendor(&self) -> String {
        "refusing test store".to_owned()
    }

    fn id(&self) -> String {
        "refusing".to_owned()
    }

    fn build(
        &self,
        _service: &str,
        _user: &str,
        _modifiers: Option<&HashMap<&str, &str>>,
    ) -> keyring_core::Result<Entry> {
        Err(Error::NoStorageAccess(Box::new(std::io::Error::other(
            "the keychain is locked",
        ))))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn persistence(&self) -> CredentialPersistence {
        CredentialPersistence::UntilDelete
    }
}

fn in_sandbox<R>(store: Arc<CredentialStore>, f: impl FnOnce(&std::path::Path) -> R) -> R {
    let config = TempDir::new().expect("config tempdir");
    let home = TempDir::new().expect("home tempdir");
    let vars: Vec<(&str, Option<String>)> = vec![
        ("HOME", Some(home.path().display().to_string())),
        ("XDG_CONFIG_HOME", Some(config.path().display().to_string())),
        ("XDG_STATE_HOME", Some(home.path().display().to_string())),
        ("XDG_CACHE_HOME", Some(home.path().display().to_string())),
        ("SP_BRIDGE_CONFIG", None),
    ];
    temp_env::with_vars(vars, || {
        keyring_core::set_default_store(store);
        f(home.path())
    })
}

fn files_named(root: &std::path::Path, name: &str) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().contains(name))
            {
                found.push(path);
            }
        }
    }
    found
}

struct LoginOutcome {
    result: Result<std::path::PathBuf, SetupError>,
    pat_files: Vec<std::path::PathBuf>,
    stored: Option<String>,
}

fn login_with_a_refusing_store() -> LoginOutcome {
    let store: Arc<CredentialStore> = Arc::new(RefusingStore);
    in_sandbox(store, |home| {
        let result = setup::login(GOOD, None).map(|paths| paths.pat_file);
        let mut pat_files = files_named(home, "pat");
        if let Some(root) = std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from) {
            pat_files.extend(files_named(&root, "pat"));
        }
        let stored = result
            .as_ref()
            .ok()
            .and_then(|path| std::fs::read_to_string(path).ok());
        LoginOutcome {
            result,
            pat_files,
            stored,
        }
    })
}

#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn on_linux_a_secret_service_that_refuses_the_probe_keeps_the_pat_in_the_file() {
    let outcome = login_with_a_refusing_store();
    let pat_file = outcome
        .result
        .expect("sign-in succeeds with the file store");
    assert_eq!(outcome.stored.as_deref(), Some(GOOD));
    assert!(
        outcome.pat_files.contains(&pat_file),
        "{:?}",
        outcome.pat_files
    );
    let store: Arc<CredentialStore> = Arc::new(RefusingStore);
    let selected = in_sandbox(store, |_| credential::select_store());
    assert_eq!(selected.ok(), Some(PatStore::File));
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[test]
fn on_macos_and_windows_a_refusing_store_fails_sign_in_and_writes_no_pat_file() {
    use systemprompt_bridge::config::gateways::credential::CredentialError;

    let outcome = login_with_a_refusing_store();
    assert_eq!(
        outcome.stored, None,
        "no PAT is stored when the keychain refuses"
    );
    let pat_files = outcome.pat_files;
    let err = outcome
        .result
        .expect_err("a refused keychain fails the sign-in");
    assert!(
        matches!(
            err,
            SetupError::Credential(CredentialError::StoreRefused(_))
        ),
        "{err:?}"
    );
    assert!(
        err.to_string().contains("unlock it"),
        "the message says what to do: {err}"
    );
    assert!(
        pat_files.is_empty(),
        "no PAT file is written when the keychain refuses: {pat_files:?}"
    );
}

#[test]
fn a_store_that_forgets_its_credentials_selects_the_file() {
    let store: Arc<CredentialStore> = keyring_core::mock::Store::new().expect("mock store");
    let selected = in_sandbox(store, |_| credential::select_store());
    assert_eq!(selected.ok(), Some(PatStore::File));
}

#[cfg(target_os = "macos")]
#[test]
fn the_macos_keychain_store_keeps_credentials_until_deleted() {
    let store = apple_native_keyring_store::keychain::Store::new().expect("keychain store");
    assert!(matches!(
        store.persistence(),
        CredentialPersistence::UntilDelete
    ));
}

#[cfg(target_os = "windows")]
#[test]
fn the_windows_credential_store_keeps_credentials_until_deleted() {
    let store = windows_native_keyring_store::Store::new().expect("credential manager store");
    assert!(matches!(
        store.persistence(),
        CredentialPersistence::UntilDelete
    ));
}
