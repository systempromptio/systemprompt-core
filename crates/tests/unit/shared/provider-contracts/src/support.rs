//! Shared helpers for provider-contract context tests.
//!
//! [`WebConfig`] has no `Default` and is `deny_unknown_fields`, so the
//! contexts under test need a fully-populated instance. We deserialize one
//! from a complete YAML literal once and clone it per test.

use systemprompt_provider_contracts::WebConfig;

pub const WEB_CONFIG_YAML: &str = r##"
paths:
  templates: templates
  assets: assets
branding:
  name: Example
  title: Example Site
  description: A test site
  copyright: "2026"
  themeColor: "#000000"
  display_sitename: true
  twitter_handle: "@example"
  logo:
    primary:
      svg: /logo.svg
  favicon: /favicon.ico
fonts:
  body:
    family: Inter
    fallback: sans-serif
  heading:
    family: Inter
    fallback: sans-serif
colors:
  light:
    primary: { hsl: "0 0% 0%", rgb: [0, 0, 0] }
    secondary: { hsl: "0 0% 50%", rgb: [128, 128, 128] }
    success: "#0f0"
    warning: "#ff0"
    error: "#f00"
    surface: { default: "#fff", dark: "#111", variant: "#eee", secondaryContainer: "#ddd", errorContainer: "#fdd" }
    text: { primary: "#000", secondary: "#333", inverted: "#fff", disabled: "#999" }
    background: { default: "#fff", dark: "#000" }
    border: { default: "#ccc", dark: "#444", outline: "#888" }
  dark:
    primary: { hsl: "0 0% 100%", rgb: [255, 255, 255] }
    secondary: { hsl: "0 0% 50%", rgb: [128, 128, 128] }
    success: "#0f0"
    warning: "#ff0"
    error: "#f00"
    surface: { default: "#111", dark: "#000", variant: "#222", secondaryContainer: "#333", errorContainer: "#411" }
    text: { primary: "#fff", secondary: "#ccc", inverted: "#000", disabled: "#666" }
    background: { default: "#000", dark: "#000" }
    border: { default: "#444", dark: "#222", outline: "#888" }
typography:
  sizes: { xs: "0.75rem", sm: "0.875rem", md: "1rem", lg: "1.25rem", xl: "1.5rem", xxl: "2rem" }
  weights: { regular: 400, medium: 500, semibold: 600, bold: 700 }
spacing: { xs: "4px", sm: "8px", md: "16px", lg: "24px", xl: "32px", xxl: "48px" }
radius: { xs: "2px", sm: "4px", md: "8px", lg: "12px", xl: "16px", xxl: "24px", round: "9999px" }
shadows:
  light: { sm: "a", md: "b", lg: "c", accent: "d" }
  dark: { sm: "a", md: "b", lg: "c", accent: "d" }
animation: { fast: "100ms", normal: "200ms", slow: "400ms" }
zIndex: { base: 0, content: 1, navigation: 10, modal: 100, tooltip: 1000 }
layout:
  headerHeight: "64px"
  sidebarLeft: { width: "240px", minWidth: "200px", maxWidth: "280px" }
  sidebarRight: { width: "240px", minWidth: "200px", maxWidth: "280px" }
  navHeight: "48px"
  contentMaxWidth: "1200px"
card:
  radius: { default: "8px", cut: "16px" }
  padding: { sm: "8px", md: "16px", lg: "24px" }
  gradient: { start: "#fff", mid: "#eee", end: "#ddd" }
mobile:
  spacing: { xs: "4px", sm: "8px", md: "16px", lg: "24px", xl: "32px", xxl: "48px" }
  typography:
    sizes: { xs: "0.75rem", sm: "0.875rem", md: "1rem", lg: "1.25rem", xl: "1.5rem", xxl: "2rem" }
  layout: { headerHeight: "56px", navHeight: "48px" }
  card:
    padding: { sm: "8px", md: "16px", lg: "24px" }
touchTargets: { default: "44px", sm: "32px", lg: "56px" }
"##;

#[must_use]
pub fn web_config() -> WebConfig {
    serde_yaml::from_str(WEB_CONFIG_YAML).expect("web config YAML deserializes")
}
