#![allow(clippy::all)]

#[cfg(test)]
mod org_plugins;

#[cfg(all(test, not(any(target_os = "windows", target_os = "macos"))))]
mod resolution;
