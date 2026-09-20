#![allow(clippy::all)]

#[cfg(test)]
mod commands;
#[cfg(test)]
mod doctor_checks;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod doctor_proxy;
