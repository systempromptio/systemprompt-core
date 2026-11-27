//! The macOS keychain sealer and keystore. The format checks never reach the
//! keychain; the round trip runs against the real login keychain with user
//! interaction disabled, so it skips rather than prompts or hangs where no
//! unlocked login keychain is usable.

use std::time::{SystemTime, UNIX_EPOCH};

use security_framework::os::macos::keychain::SecKeychain;
use sp_secure::{KeychainSealer, Keystore, Sealer, SecureError};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn unique_app_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    format!(
        "io.systemprompt.sp-secure-tests.{}.{nanos}",
        std::process::id()
    )
}

struct RemovesKey<'a>(&'a KeychainSealer);

impl Drop for RemovesKey<'_> {
    fn drop(&mut self) {
        // Why: discard-ok: cleanup of a test key; the test already asserted.
        self.0.delete_key().ok();
    }
}

#[test]
fn empty_and_foreign_ciphertexts_fail_before_the_keychain() {
    let sealer = KeychainSealer::new(&unique_app_id());
    assert!(matches!(sealer.open(b""), Err(SecureError::Empty)));
    assert!(matches!(
        sealer.open(b"spmem01\0not ours"),
        Err(SecureError::Rejected { .. })
    ));
    assert!(matches!(
        sealer.open(b"spkc1\0an earlier key-pair format"),
        Err(SecureError::Rejected { .. })
    ));
    assert!(matches!(
        sealer.open(b"spkc2\0short"),
        Err(SecureError::Rejected { .. })
    ));
    assert!(format!("{sealer:?}").contains("key_loaded: false"));
}

#[test]
fn debug_names_the_key_label_and_nothing_else() {
    let sealer = KeychainSealer::new("io.example.app");
    let debug = format!("{sealer:?}");
    assert!(debug.contains("io.example.app sealing key"));
    assert!(debug.contains("key_loaded: false"));
}

#[test]
fn login_keychain_seals_opens_and_stores() -> TestResult {
    let Ok(_no_prompts) = SecKeychain::disable_user_interaction() else {
        // skip-ok: user interaction cannot be disabled, so a keychain call could prompt
        return Ok(());
    };
    if SecKeychain::default().is_err() {
        // skip-ok: this session has no default (login) keychain
        return Ok(());
    }
    let app_id = unique_app_id();
    let sealer = KeychainSealer::new(&app_id);
    let _cleanup = RemovesKey(&sealer);
    let ciphertext = match sealer.seal(b"sp_pat_secret") {
        Ok(ciphertext) => ciphertext,
        // skip-ok: the login keychain is locked or would need the user
        Err(error) if error.keeps_ciphertext() => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    assert!(!ciphertext.windows(13).any(|w| w == b"sp_pat_secret"));
    assert_eq!(sealer.open(&ciphertext)?.as_slice(), b"sp_pat_secret");
    assert_ne!(sealer.seal(b"sp_pat_secret")?, ciphertext);
    assert!(sealer.open(&sealer.seal(b"")?)?.is_empty());
    assert!(sealer.available());

    let restarted = KeychainSealer::new(&app_id);
    assert_eq!(restarted.open(&ciphertext)?.as_slice(), b"sp_pat_secret");

    let other_build = KeychainSealer::new(&format!("{app_id}.canary"));
    assert!(matches!(
        other_build.open(&ciphertext),
        Err(SecureError::Rejected { .. })
    ));

    let mut tampered = ciphertext.clone();
    if let Some(last) = tampered.last_mut() {
        *last ^= 0x01;
    }
    assert!(matches!(
        restarted.open(&tampered),
        Err(SecureError::Rejected { .. })
    ));

    let store = Keystore::platform()?;
    assert!(store.load(&app_id, "client")?.is_none());
    store.store(&app_id, "client", b"first")?;
    store.store(&app_id, "client", b"second")?;
    assert_eq!(
        store.load(&app_id, "client")?.as_deref().map(Vec::as_slice),
        Some(b"second".as_slice())
    );
    store.delete(&app_id, "client")?;
    store.delete(&app_id, "client")?;
    assert!(store.load(&app_id, "client")?.is_none());

    sealer.delete_key()?;
    assert!(matches!(
        KeychainSealer::new(&app_id).open(&ciphertext),
        Err(SecureError::Rejected { .. })
    ));
    Ok(())
}
