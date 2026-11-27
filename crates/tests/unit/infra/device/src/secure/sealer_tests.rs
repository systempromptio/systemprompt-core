use systemprompt_device::secure::{MemorySealer, Sealer, SecureError};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn memory_sealer_round_trips() -> TestResult {
    let sealer = MemorySealer::for_tests();
    assert!(sealer.available());
    let ciphertext = sealer.seal(b"sp_pat_secret")?;
    assert!(!ciphertext.windows(13).any(|w| w == b"sp_pat_secret"));
    assert_eq!(sealer.open(&ciphertext)?.as_slice(), b"sp_pat_secret");
    Ok(())
}

#[test]
fn memory_sealer_round_trips_empty_plaintext() -> TestResult {
    let sealer = MemorySealer::for_tests();
    let ciphertext = sealer.seal(b"")?;
    assert!(sealer.open(&ciphertext)?.is_empty());
    Ok(())
}

#[test]
fn another_sealer_rejects_the_ciphertext() -> TestResult {
    let first = MemorySealer::for_tests();
    let second = MemorySealer::for_tests();
    let ciphertext = first.seal(b"secret")?;
    assert!(matches!(
        second.open(&ciphertext),
        Err(SecureError::Rejected { .. })
    ));
    Ok(())
}

#[test]
fn garbage_is_rejected_and_empty_is_empty() {
    let sealer = MemorySealer::for_tests();
    assert!(matches!(
        sealer.open(b"garbage"),
        Err(SecureError::Rejected { .. })
    ));
    assert!(matches!(sealer.open(b""), Err(SecureError::Empty)));
}

#[test]
fn a_forged_handle_is_rejected() -> TestResult {
    let sealer = MemorySealer::for_tests();
    let mut forged = sealer.seal(b"secret")?;
    let handle_start = forged.len() - 16;
    forged[handle_start..].fill(0);
    assert!(matches!(
        sealer.open(&forged),
        Err(SecureError::Rejected { .. })
    ));
    let truncated = &forged[..handle_start];
    assert!(matches!(
        sealer.open(truncated),
        Err(SecureError::Rejected { .. })
    ));
    Ok(())
}

#[test]
fn disabled_sealer_is_unavailable_and_keeps_ciphertext() {
    let sealer = MemorySealer::disabled();
    assert!(!sealer.available());
    let ciphertext = sealer.seal(b"secret");
    assert!(matches!(ciphertext, Err(SecureError::Unsupported)));
    let opened = sealer.open(b"anything");
    assert!(opened.as_ref().is_err_and(SecureError::keeps_ciphertext));
}

#[cfg(not(any(windows, target_os = "macos")))]
#[test]
fn platform_sealer_without_a_native_store_is_unavailable() {
    assert!(!systemprompt_device::secure::platform_sealer("io.example.app").available());
}

#[test]
fn debug_does_not_show_secrets() -> TestResult {
    let sealer = MemorySealer::for_tests();
    sealer.seal(b"sp_pat_visible")?;
    assert!(!format!("{sealer:?}").contains("sp_pat_visible"));
    Ok(())
}
