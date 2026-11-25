//! The member's claim on the composition's one site fallback.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::Contributions;

impl Contributions<'_> {
    pub fn site_fallback(&mut self, f: crate::router::SiteFallback) -> &mut Self {
        self.sink.site_fallbacks.push(f);
        self
    }
}
