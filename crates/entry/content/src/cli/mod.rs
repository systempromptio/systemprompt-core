//! The content capability's commands: `core content` ([`content`]) and
//! `web sitemap` ([`sitemap`]). Each registers under its kernel parent
//! group through `register_cli_command!`, so the CLI assembles them without
//! naming this crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod content;
pub mod sitemap;
