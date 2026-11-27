use systemprompt_device::secure::{MemorySealer, SealedString, SecureError};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn seals_and_opens_a_string() -> TestResult {
    let sealer = MemorySealer::for_tests();
    let ciphertext = SealedString::seal(&sealer, "sp_pat_123")?;
    assert_eq!(ciphertext.open(&sealer)?.as_str(), "sp_pat_123");
    Ok(())
}

#[test]
fn serde_round_trip_keeps_the_ciphertext() -> TestResult {
    let sealer = MemorySealer::for_tests();
    let ciphertext = SealedString::seal(&sealer, "sp_pat_123")?;
    let json = serde_json::to_string(&ciphertext)?;
    assert!(!json.contains("sp_pat_123"));
    let back: SealedString = serde_json::from_str(&json)?;
    assert_eq!(back, ciphertext);
    assert_eq!(back.open(&sealer)?.as_str(), "sp_pat_123");
    Ok(())
}

#[test]
fn deserialising_rejects_bad_base64_and_empty() {
    assert!(serde_json::from_str::<SealedString>("\"not base64!\"").is_err());
    assert!(serde_json::from_str::<SealedString>("\"\"").is_err());
    assert!(matches!(
        SealedString::from_base64("%%%".to_owned()),
        Err(SecureError::Encoding { .. })
    ));
}

#[test]
fn non_utf8_plaintext_fails_open_as_string() -> TestResult {
    let sealer = MemorySealer::for_tests();
    let ciphertext = SealedString::seal_bytes(&sealer, &[0xff, 0xfe])?;
    assert!(matches!(
        ciphertext.open(&sealer),
        Err(SecureError::NotUtf8)
    ));
    assert_eq!(ciphertext.open_bytes(&sealer)?.as_slice(), &[0xff, 0xfe]);
    Ok(())
}

#[test]
fn debug_hides_the_text() -> TestResult {
    let sealer = MemorySealer::for_tests();
    let ciphertext = SealedString::seal(&sealer, "x")?;
    assert!(!format!("{ciphertext:?}").contains(ciphertext.as_base64()));
    Ok(())
}
