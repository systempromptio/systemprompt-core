//! Projects a verified manifest into the harness-owned config dir, and into the
//! in-memory [`Applied`] snapshot the governance and sync crates read.
//!
//! Planning is pure and total: [`plan`] never fails and never panics on an odd
//! manifest; an entry it cannot take is dropped and named in
//! [`Projection::warnings`]. Nothing is projected for a host the manifest does
//! not enable, so applying that projection clears what an earlier one wrote.
//! [`digest`] fingerprints everything a projection depends on, so a re-signed
//! but otherwise identical manifest is recognised and applied as nothing, and
//! [`wanted_bundle_files`] names the plugin bundle files a projection reads,
//! for the sync engine to download. [`Applier`] writes a projection into one
//! config dir and removes what an earlier one wrote and this one no longer
//! does; its methods block on the filesystem, so async callers run them on a
//! blocking thread.
//!
//! The applier owns, under the config dir, only what a ledger records:
//! `skills/<dir>/SKILL.md` (listed in `skills/.systemprompt-managed.json`) and
//! the runtime files placed beside a skill (listed in that skill's
//! `.systemprompt-runtime.json`, removed with its directory); the
//! `agents/<name>.md` and `themes/<name>.json` files of plugin bundles (listed
//! in `.systemprompt-owned.json`); the block between [`RULES_BEGIN`] and
//! [`RULES_END`] in `AGENTS.md`. Marketplace defaults go into the global
//! settings file only where the user has not set the key, and are never
//! removed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod apply;
mod bundle;
mod defaults;
mod digest;
mod error;
mod gateway;
mod input;
mod ledger;
mod managed;
mod mcp;
mod model;
mod plan;
mod readback;
mod rules;
mod runtime;
mod skill_md;

pub use apply::{Applier, ApplyReport};
pub use digest::{ContentDigest, digest};
pub use error::{MaterializeError, ProjectionError};
pub use gateway::DEFAULT_MODEL_FAMILIES;
pub use input::{BundleFiles, ProjectionInput};
pub use ledger::{
    AGENTS_FILE, OWNED_LEDGER, RUNTIME_LEDGER, SKILL_FILE, SKILLS_DIR, SKILLS_LEDGER,
};
pub use model::{
    Applied, BundleFile, Counts, DefaultTheme, Gateway, GatewayModel, ManagedMcp, Projection,
    Protocol, SkillFile, SkillPublication, SkillSet,
};
pub use plan::{plan, wanted_bundle_files};
pub use rules::{RULES_BEGIN, RULES_END};
