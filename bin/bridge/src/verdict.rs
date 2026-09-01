//! The tone every state on the GUI wire ships beside itself.
//!
//! A state enum tells the front end *what* happened; a [`Verdict`] tells it
//! *how that reads* — green, amber, red, or not yet known — and it is computed
//! here, once, next to the enum. The front end renders the tone and looks the
//! code up in the catalogue; it never decides for itself what a state means.
//!
//! That rule exists because it was broken: a GUI card tested the MCP auth
//! state's *name* against a variant that did not exist and told users four
//! healthy servers were broken, while another pane, reading the same
//! snapshot, said they were fine. Two panes, two derivations, two answers.
//! `scripts/lint-bridge-verdicts.sh` keeps the derivation out of JavaScript.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;

/// How a state reads, ordered so that `max` is "worst".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub enum Tone {
    Ok,
    Unknown,
    Probing,
    Warn,
    Err,
}

impl Tone {
    #[must_use]
    pub fn worst(self, other: Self) -> Self {
        self.max(other)
    }

    #[must_use]
    pub fn fold<I: IntoIterator<Item = Self>>(tones: I, empty: Self) -> Self {
        tones.into_iter().reduce(Self::worst).unwrap_or(empty)
    }
}

/// A state's tone and its own serialised code, side by side on the wire.
///
/// `C` is the state enum itself, so the code the front end looks up is the
/// one serde already spells — there is no second list of names to drift.
#[derive(Debug, Clone, Copy, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct Verdict<C> {
    pub tone: Tone,
    pub code: C,
}

impl<C> Verdict<C> {
    #[must_use]
    pub const fn new(tone: Tone, code: C) -> Self {
        Self { tone, code }
    }
}
