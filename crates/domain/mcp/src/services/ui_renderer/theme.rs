//! Deployment-supplied theming for rendered artifact UI.
//!
//! Every renderer's stylesheet addresses colour, radius, shadow, and type
//! through the `--mcpui-*` custom properties declared in
//! `assets/css/tokens.css`. A deployment that wants artifacts to look like the
//! rest of its product registers an [`ArtifactTheme`] re-declaring some or all
//! of them; no renderer has to be forked to restyle it.
//!
//! `tokens` lands in a `:root` block after the built-in tokens and before any
//! renderer CSS, so a partial theme overrides what it names and inherits the
//! rest. `extra_css` is appended last, for what a custom property cannot
//! express — an `@font-face`, a backdrop filter, a rule aimed at one renderer.
//!
//! A member contributes its theme with
//! `c.slot::<UiSlot>().theme(..)` ([`UiSlot`](crate::contract::UiSlot)); the
//! built [`UiRendererRegistry`](super::UiRendererRegistry) applies it to every
//! document it renders. Two themes do not compose, so the slot refuses a
//! second one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::templates::html::{THEME_CSS_ANCHOR, THEME_TOKENS_ANCHOR};

/// A deployment's re-declaration of the `--mcpui-*` design tokens.
#[derive(Debug, Clone, Copy)]
pub struct ArtifactTheme {
    pub tokens: &'static str,
    pub extra_css: &'static str,
}

impl ArtifactTheme {
    pub const fn new(tokens: &'static str) -> Self {
        Self {
            tokens,
            extra_css: "",
        }
    }

    pub const fn with_extra_css(mut self, css: &'static str) -> Self {
        self.extra_css = css;
        self
    }

    pub(crate) fn apply(&self, html: &str) -> String {
        let tokens = format!("{THEME_TOKENS_ANCHOR}\n<style>\n:root {{\n{}\n}}\n</style>", self.tokens);
        let themed = html.replacen(THEME_TOKENS_ANCHOR, &tokens, 1);
        if self.extra_css.is_empty() {
            return themed;
        }
        let extra = format!("<style>\n{}\n</style>\n{THEME_CSS_ANCHOR}", self.extra_css);
        themed.replacen(THEME_CSS_ANCHOR, &extra, 1)
    }
}
