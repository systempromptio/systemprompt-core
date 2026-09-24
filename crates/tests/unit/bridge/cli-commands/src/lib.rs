#![allow(clippy::all)]

#[cfg(test)]
mod commands;
#[cfg(test)]
mod doctor_checks;
#[cfg(test)]
mod doctor_claude_code_routing;
#[cfg(test)]
mod doctor_opencode;
#[cfg(all(test, not(any(target_os = "macos", target_os = "windows"))))]
mod doctor_proxy;
