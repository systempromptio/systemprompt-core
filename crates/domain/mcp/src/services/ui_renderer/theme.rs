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
//! Registration is compile-time, the same shape the safety-scanner and
//! route-selector registries use, so the branding is linked in by the extension
//! that owns it rather than configured at runtime.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
}

/// Compile-time registration of an [`ArtifactTheme`].
///
/// Two registered themes do not compose — both re-declaring `--mcpui-accent`
/// has no meaningful winner — so [`active_theme`] picks the last by name and
/// warns rather than blending them.
#[derive(Debug, Clone, Copy)]
pub struct ArtifactThemeRegistration {
    pub name: &'static str,
    pub factory: fn() -> ArtifactTheme,
}

inventory::collect!(ArtifactThemeRegistration);

#[macro_export]
macro_rules! register_artifact_theme {
    ($factory:expr, name = $name:expr $(,)?) => {
        ::inventory::submit! {
            $crate::services::ui_renderer::ArtifactThemeRegistration {
                name: $name,
                factory: $factory,
            }
        }
    };
}

pub fn active_theme() -> Option<ArtifactTheme> {
    // Why: `inventory` makes no ordering promise across link units, so sort to
    // keep one binary rendering the same way run to run.
    let mut found: Vec<&ArtifactThemeRegistration> =
        inventory::iter::<ArtifactThemeRegistration>().collect();
    found.sort_by_key(|registration| registration.name);

    let last = found.last()?;
    if found.len() > 1 {
        let names: Vec<&str> = found.iter().map(|registration| registration.name).collect();
        tracing::warn!(
            themes = ?names,
            chosen = last.name,
            "more than one artifact theme registered; the last by name wins"
        );
    }
    Some((last.factory)())
}
