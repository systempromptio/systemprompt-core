#![allow(clippy::all)]

#[cfg(test)]
mod auth_chain;
#[cfg(test)]
mod host_registries;
#[cfg(all(test, any(target_os = "windows", target_os = "macos")))]
mod marketplace_source;
