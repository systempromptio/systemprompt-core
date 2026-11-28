//! The eight behaviours of the harness's live test package, re-expressed
//! against the scripted instance: no real instance, no model, loopback only.
//!
//! Each test starts its own scripted instance, signs a fresh data root in
//! through the headless `login` (which pins the manifest key), and exercises
//! the same harness and device code the live tests did: discovery and the PAT
//! exchange, the heartbeat, the signed manifest and its application, the model
//! list, the hook routes, the Brain probe, the notifications feed, and one
//! governed step through the headless `run`.

#[cfg(test)]
mod engine_tests;
#[cfg(test)]
mod feeds_tests;
#[cfg(test)]
mod fixture;
#[cfg(test)]
mod hooks_tests;
#[cfg(test)]
mod manifest_tests;
#[cfg(test)]
mod session_tests;
