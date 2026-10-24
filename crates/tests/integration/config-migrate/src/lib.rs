//! Golden proof for `systemprompt admin config migrate` (credential.md §9.1).
//!
//! Each fixture under `common/fixtures/catalog/<install>/legacy/` is migrated
//! into a scratch services tree, loaded through the real loader, resolved,
//! and compared with the 0.72.1 snapshot in `golden/resolution.json`, with an
//! explicit allowlist of intended differences. A second migrate writes
//! nothing; the legacy tree itself is refused with the migrate hint; every
//! undecidable shape aborts naming its file and keys.

#[cfg(test)]
mod aborts;
#[cfg(test)]
mod data;
#[cfg(test)]
mod golden;
#[cfg(test)]
mod legacy_wire;
#[cfg(test)]
mod roles;
#[cfg(test)]
mod support;
