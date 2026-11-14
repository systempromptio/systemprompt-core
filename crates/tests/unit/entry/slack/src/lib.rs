//! Unit tests for the `systemprompt-slack-entry` crate: the extension
//! registration surface, the offline extension-config preflight, and the
//! route's form helpers.

#![allow(clippy::all)]

#[cfg(test)]
mod extension;
#[cfg(test)]
mod extension_config_preflight;
#[cfg(test)]
mod http_forms;
