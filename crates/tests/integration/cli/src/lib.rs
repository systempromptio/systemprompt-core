//! Integration tests for systemprompt-cli covering both the public library
//! surface (in-process tests for `presentation`, `cloud`, `shared`,
//! `session`, `paths`) and the `systemprompt` binary itself (subprocess
//! tests via `assert_cmd::cargo_bin`). The coverage harness builds
//! `--workspace --lib --bins`, the cli crate exposes `[[bin]] systemprompt`,
//! and child processes inherit `LLVM_PROFILE_FILE` from the test runner so
//! the binary's profraw lands in the merged report.

#![allow(clippy::all)]

#[cfg(test)]
pub(crate) mod env_lock {
    use std::sync::Mutex;
    // Process-global lock for any test that mutates env vars ($HOME,
    // SYSTEMPROMPT_PROFILE, SYSTEMPROMPT_SERVICES_PATH). CLI code resolves
    // these at call time, so parallel tests racing them produce
    // non-deterministic failures.
    pub(crate) static ENV: Mutex<()> = Mutex::new(());
}

#[cfg(test)]
mod cloud_dockerfile_validation;
#[cfg(test)]
mod cloud_profile_templates;
#[cfg(test)]
mod cloud_tenant_helpers;
#[cfg(test)]
mod cloud_tenant_validation;
#[cfg(test)]
mod presentation_renderer;
#[cfg(test)]
mod session_store;
#[cfg(test)]
mod shared_profile;
#[cfg(test)]
mod shared_project;
