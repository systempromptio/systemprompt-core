//! Tests for `systemprompt_device::platform`, through its public API. The
//! tests of one system's implementation are compiled on that system only.

mod data_root_tests;
#[cfg(target_os = "linux")]
mod dirs_linux_tests;
#[cfg(target_os = "macos")]
mod dirs_macos_tests;
#[cfg(windows)]
mod dirs_windows_tests;
#[cfg(target_os = "macos")]
mod keychain_tests;
#[cfg(target_os = "linux")]
mod keyring_tests;
#[cfg(unix)]
mod owner_only_tests;
