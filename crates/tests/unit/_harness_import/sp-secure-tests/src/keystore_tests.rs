use sp_secure::Keystore;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn memory_store_round_trips_and_deletes() -> TestResult {
    let store = Keystore::memory_for_tests();
    assert!(store.load("svc", "client")?.is_none());
    store.store("svc", "client", b"secret")?;
    assert_eq!(
        store.load("svc", "client")?.as_deref().map(Vec::as_slice),
        Some(b"secret".as_slice())
    );
    store.delete("svc", "client")?;
    store.delete("svc", "client")?;
    assert!(store.load("svc", "client")?.is_none());
    Ok(())
}

#[cfg(not(any(windows, target_os = "macos")))]
#[test]
fn platform_store_is_unsupported_without_a_native_store() {
    assert!(matches!(
        Keystore::platform(),
        Err(sp_secure::SecureError::Unsupported)
    ));
}
