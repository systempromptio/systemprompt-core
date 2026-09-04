#![allow(clippy::all)]

// The asset table moved out of the GUI cfg gate (it is pure string handling
// with no platform dependency), so these run everywhere — including Linux CI,
// where the rest of this crate compiles out and the web assets previously got
// no verification on a PR at all.
#[cfg(test)]
mod assets;
#[cfg(all(test, any(target_os = "windows", target_os = "macos")))]
mod cancel_scopes;
#[cfg(all(test, any(target_os = "windows", target_os = "macos")))]
mod host_model_view;
#[cfg(all(test, any(target_os = "windows", target_os = "macos")))]
mod ipc;
#[cfg(all(test, any(target_os = "windows", target_os = "macos")))]
mod jwt;
#[cfg(all(test, any(target_os = "windows", target_os = "macos")))]
mod marketplace_children;
#[cfg(all(test, any(target_os = "windows", target_os = "macos")))]
mod marketplace_hooks;
#[cfg(all(test, any(target_os = "windows", target_os = "macos")))]
mod profile;
#[cfg(all(test, any(target_os = "windows", target_os = "macos")))]
mod server_json;
