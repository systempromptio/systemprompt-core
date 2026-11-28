//! The Linux keyring sealer and credential store. The format checks never
//! reach a keyring; the round trip runs against whichever tier the machine
//! has (the Secret Service, the kernel keyring, or neither), and holds on each.

use std::time::{SystemTime, UNIX_EPOCH};

use systemprompt_device::platform::KeyringSealer;
use systemprompt_device::secure::{Keystore, Sealer, SecureError, platform_sealer};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const PROCESS_TIER: &[u8] = b"sppm1\0";

fn unique_app_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    format!(
        "io.systemprompt.sp-secure-tests.{}.{nanos}",
        std::process::id()
    )
}

struct RemovesKey<'a>(&'a KeyringSealer);

impl Drop for RemovesKey<'_> {
    fn drop(&mut self) {
        // Why: discard-ok: cleanup of a test key; the test already asserted.
        self.0.delete_key().ok();
    }
}

#[test]
fn empty_and_foreign_ciphertexts_fail_before_a_keyring() {
    let sealer = KeyringSealer::new(&unique_app_id());
    assert!(matches!(sealer.open(b""), Err(SecureError::Empty)));
    assert!(matches!(
        sealer.open(b"spmem01\0not ours"),
        Err(SecureError::Rejected { .. })
    ));
    assert!(matches!(
        sealer.open(b"spkc2\0a keychain ciphertext"),
        Err(SecureError::Rejected { .. })
    ));
    assert!(matches!(
        sealer.open(b"sppm1\0short"),
        Err(SecureError::Rejected { .. })
    ));
    assert!(format!("{sealer:?}").contains("sealing: None"));
}

#[test]
fn a_process_ciphertext_of_another_process_is_rejected() {
    let sealer = KeyringSealer::new(&unique_app_id());
    let mut foreign = PROCESS_TIER.to_vec();
    foreign.extend_from_slice(&[0u8; 12 + 16 + 4]);
    assert!(matches!(
        sealer.open(&foreign),
        Err(SecureError::Rejected { .. })
    ));
}

#[test]
fn debug_names_the_key_label_and_no_key() {
    let sealer = KeyringSealer::new("io.example.app");
    let debug = format!("{sealer:?}");
    assert!(debug.contains("io.example.app sealing key"));
    assert!(debug.contains("sealing: None"));
}

#[test]
fn the_sealer_seals_and_opens_on_whichever_tier_the_machine_has() -> TestResult {
    let app_id = unique_app_id();
    let sealer = KeyringSealer::new(&app_id);
    let _cleanup = RemovesKey(&sealer);
    let ciphertext = sealer.seal(b"sp_pat_secret")?;
    assert!(!ciphertext.windows(13).any(|w| w == b"sp_pat_secret"));
    assert_eq!(sealer.open(&ciphertext)?.as_slice(), b"sp_pat_secret");
    assert_ne!(sealer.seal(b"sp_pat_secret")?, ciphertext);
    assert!(sealer.open(&sealer.seal(b"")?)?.is_empty());
    assert!(sealer.available());

    let mut tampered = ciphertext.clone();
    if let Some(last) = tampered.last_mut() {
        *last ^= 0x01;
    }
    assert!(matches!(
        sealer.open(&tampered),
        Err(SecureError::Rejected { .. })
    ));

    let other_build = KeyringSealer::new(&format!("{app_id}.canary"));
    assert!(matches!(
        other_build.open(&ciphertext),
        Err(SecureError::Rejected { .. })
    ));

    let restarted = KeyringSealer::new(&app_id);
    match restarted.open(&ciphertext) {
        Ok(opened) => assert_eq!(opened.as_slice(), b"sp_pat_secret"),
        Err(SecureError::Rejected { .. }) => assert!(ciphertext.starts_with(PROCESS_TIER)),
        Err(error) => return Err(error.into()),
    }

    sealer.delete_key()?;
    assert!(matches!(
        KeyringSealer::new(&app_id).open(&ciphertext),
        Err(SecureError::Rejected { .. })
    ));
    Ok(())
}

#[test]
fn the_platform_sealer_is_always_available() {
    let app_id = unique_app_id();
    let cleanup = KeyringSealer::new(&app_id);
    let _cleanup = RemovesKey(&cleanup);
    assert!(platform_sealer(&app_id).available());
}

#[test]
fn the_platform_store_round_trips_or_reports_itself_unavailable() -> TestResult {
    let store = match Keystore::platform() {
        Ok(store) => store,
        // skip-ok: this machine has neither a Secret Service nor a kernel keyring
        Err(error) if error.keeps_ciphertext() => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let service = unique_app_id();
    match store.store(&service, "client", b"first") {
        Ok(()) => {},
        // skip-ok: the keyring is locked or refuses writes in this session
        Err(error) if error.keeps_ciphertext() => return Ok(()),
        Err(error) => return Err(error.into()),
    }
    store.store(&service, "client", b"second")?;
    assert_eq!(
        store
            .load(&service, "client")?
            .as_deref()
            .map(Vec::as_slice),
        Some(b"second".as_slice())
    );
    store.delete(&service, "client")?;
    store.delete(&service, "client")?;
    Ok(())
}
