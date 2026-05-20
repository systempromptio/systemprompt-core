//! Unit tests for systemprompt-security crate.

#![allow(clippy::all)]

#[cfg(test)]
mod extraction;
#[cfg(test)]
mod manifest_signing_jcs;
#[cfg(test)]
mod rs256_cutover;
#[cfg(test)]
mod services;
#[cfg(test)]
mod signing_key_independence;
