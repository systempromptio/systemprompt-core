//! Tests for sp-secure, through its public API.

#[cfg(test)]
mod compare_tests;
#[cfg(test)]
mod contained_path_tests;
#[cfg(test)]
mod files_tests;
#[cfg(all(test, target_os = "macos"))]
mod keychain_tests;
#[cfg(test)]
mod keystore_tests;
#[cfg(test)]
mod sealed_tests;
#[cfg(test)]
mod sealer_tests;
