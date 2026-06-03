#![allow(clippy::all)]

#[cfg(all(test, any(target_os = "windows", target_os = "macos")))]
mod assets;
#[cfg(all(test, any(target_os = "windows", target_os = "macos")))]
mod host_model_view;
#[cfg(all(test, any(target_os = "windows", target_os = "macos")))]
mod ipc;
#[cfg(all(test, any(target_os = "windows", target_os = "macos")))]
mod jwt;
#[cfg(all(test, any(target_os = "windows", target_os = "macos")))]
mod marketplace_hooks;
#[cfg(all(test, any(target_os = "windows", target_os = "macos")))]
mod profile;
#[cfg(all(test, any(target_os = "windows", target_os = "macos")))]
mod server_json;
