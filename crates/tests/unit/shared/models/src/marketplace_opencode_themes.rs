//! The kit-side `opencode:` block: several themes with a default, the
//! single-theme form older kits write, and the validation that keeps a
//! default pointing at a declared theme.

use systemprompt_manifest::services::MarketplaceConfigFile;
use systemprompt_models::bridge::manifest::ThemeMode;

fn parse(opencode: &str) -> Result<MarketplaceConfigFile, serde_yaml::Error> {
    serde_yaml::from_str(&format!(
        r#"
marketplace:
  id: brand-kit
  name: Brand
  description: brand
  version: "1.0.0"
  enabled: true
  author:
    name: Brand
    email: brand@example.invalid
  license: MIT
  visibility: public
  plugins:
    include: []
  opencode:
{opencode}
"#
    ))
}

const SHA_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SHA_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

#[test]
fn several_themes_with_a_default_and_a_mode_validate() {
    let file = parse(&format!(
        "    themes:\n      - {{name: brand, sha256: {SHA_A}}}\n      - {{name: brand-black, sha256: {SHA_B}}}\n    default_theme: brand-black\n    theme_mode: system"
    ))
    .expect("parses");
    let opencode = file.marketplace.opencode.as_ref().expect("opencode");
    assert_eq!(opencode.themes.len(), 2);
    assert_eq!(opencode.theme_mode, Some(ThemeMode::System));
    file.marketplace.validate("brand-kit").expect("valid");
}

#[test]
fn the_single_theme_form_is_the_only_theme_and_the_default() {
    let file = parse(&format!("    theme: {{name: brand, sha256: {SHA_A}}}")).expect("parses");
    let opencode = file.marketplace.opencode.expect("opencode");
    assert_eq!(opencode.themes.len(), 1);
    assert_eq!(opencode.default_theme.as_deref(), Some("brand"));
}

#[test]
fn theme_and_themes_together_are_refused() {
    let err = parse(&format!(
        "    theme: {{name: brand, sha256: {SHA_A}}}\n    themes:\n      - {{name: other, sha256: {SHA_B}}}"
    ))
    .expect_err("ambiguous");
    assert!(err.to_string().contains("not both"), "{err}");
}

#[test]
fn a_default_theme_that_is_not_declared_is_refused() {
    let file = parse(&format!(
        "    themes:\n      - {{name: brand, sha256: {SHA_A}}}\n    default_theme: missing"
    ))
    .expect("parses");
    assert!(file.marketplace.validate("brand-kit").is_err());
}

#[test]
fn a_theme_declared_twice_is_refused() {
    let file = parse(&format!(
        "    themes:\n      - {{name: brand, sha256: {SHA_A}}}\n      - {{name: brand, sha256: {SHA_B}}}"
    ))
    .expect("parses");
    assert!(file.marketplace.validate("brand-kit").is_err());
}

#[test]
fn theme_is_no_longer_a_cli_default() {
    let file = parse("    cli_defaults:\n      theme: dark").expect("parses");
    assert!(file.marketplace.validate("brand-kit").is_err());
}

#[test]
fn unknown_opencode_keys_are_still_refused() {
    assert!(parse("    colour: red").is_err());
}
