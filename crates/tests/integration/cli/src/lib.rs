//! Integration tests for systemprompt-cli that drive the public CLI surface
//! in-process.
//!
//! The coverage harness (`just coverage`) runs `cargo test --workspace --lib`
//! and only library targets are instrumented. That makes subprocess testing
//! via `assert_cmd::cargo_bin("systemprompt")` unsuitable for raising line
//! coverage: there is no `systemprompt` binary in the workspace (the CLI is
//! shipped as `systemprompt_cli::run()` from the facade), and `--lib` builds
//! no `[[bin]]` targets even if there were. So these tests exercise the same
//! private subtrees (`runner/`, `presentation/`, `commands/cloud/**`) through
//! the `pub` surface re-exported from `crates/entry/cli/src/lib.rs`.

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
