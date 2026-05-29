//! Coverage for SiteI18nConfig validation, component partials, and
//! ExtendedData construction.

use serde_json::json;
use systemprompt_identifiers::LocaleCode;
use systemprompt_provider_contracts::{
    ExtendedData, PartialSource, PartialTemplate, RenderedComponent, SiteI18nConfig,
};

#[test]
fn i18n_default_is_english_only() {
    let cfg = SiteI18nConfig::default();
    assert_eq!(cfg.default_locale.as_str(), "en");
    assert_eq!(cfg.supported_locales, vec![LocaleCode::new("en")]);
    assert!(cfg.validate().is_ok());
}

#[test]
fn i18n_validate_rejects_default_not_in_supported() {
    let cfg = SiteI18nConfig {
        default_locale: LocaleCode::new("fr"),
        supported_locales: vec![LocaleCode::new("en")],
    };
    let err = cfg.validate().unwrap_err();
    assert!(err.contains("fr"));
    assert!(err.contains("not in supported_locales"));
}

#[test]
fn i18n_locale_prefix_empty_for_default() {
    let cfg = SiteI18nConfig::default();
    assert_eq!(cfg.locale_prefix(&LocaleCode::new("en")), "");
}

#[test]
fn i18n_locale_prefix_slashed_for_non_default() {
    let cfg = SiteI18nConfig {
        default_locale: LocaleCode::new("en"),
        supported_locales: vec![LocaleCode::new("en"), LocaleCode::new("de")],
    };
    assert_eq!(cfg.locale_prefix(&LocaleCode::new("de")), "/de");
}

#[test]
fn i18n_serde_roundtrip() {
    let cfg = SiteI18nConfig::default();
    let json = serde_json::to_string(&cfg).unwrap();
    let back: SiteI18nConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(back.default_locale, cfg.default_locale);
    assert_eq!(back.supported_locales, cfg.supported_locales);
}

#[test]
fn partial_template_embedded() {
    let t = PartialTemplate::embedded("header", "<header></header>");
    assert_eq!(t.name, "header");
    assert!(matches!(t.source, PartialSource::Embedded(c) if c == "<header></header>"));
}

#[test]
fn partial_template_file() {
    let t = PartialTemplate::file("footer", "/tmp/footer.html");
    assert_eq!(t.name, "footer");
    assert!(matches!(t.source, PartialSource::File(p) if p.ends_with("footer.html")));
}

#[test]
fn rendered_component_new_assigns_fields() {
    let c = RenderedComponent::new("nav_html", "<nav></nav>");
    assert_eq!(c.variable_name, "nav_html");
    assert_eq!(c.html, "<nav></nav>");
}

#[test]
fn extended_data_new_defaults_priority_100() {
    let d = ExtendedData::new(json!({"k": "v"}));
    assert_eq!(d.priority, 100);
    assert_eq!(d.variables["k"], "v");
}

#[test]
fn extended_data_with_priority_overrides() {
    let d = ExtendedData::with_priority(json!({}), 5);
    assert_eq!(d.priority, 5);
}
