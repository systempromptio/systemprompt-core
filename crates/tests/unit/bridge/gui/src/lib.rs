#![allow(clippy::all)]

#[cfg(all(test, any(target_os = "windows", target_os = "macos")))]
mod host_model_view;
#[cfg(all(test, any(target_os = "windows", target_os = "macos")))]
mod marketplace_hooks;
