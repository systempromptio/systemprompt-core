//! Headless credential store for the bridge suites that touch the keyring.

pub fn install_headless_keystore() {
    static INSTALL: std::sync::Once = std::sync::Once::new();
    INSTALL.call_once(|| {
        #[cfg(target_os = "linux")]
        keyring_core::set_default_store(
            linux_keyutils_keyring_store::Store::new().expect("keyutils store"),
        );
        #[cfg(not(target_os = "linux"))]
        keyring_core::set_default_store(keyring_core::mock::Store::new().expect("mock store"));
    });
}
