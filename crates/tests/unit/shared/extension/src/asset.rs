use std::path::PathBuf;

use systemprompt_extension::{AssetDefinition, AssetDefinitionBuilder, AssetType};

#[test]
fn asset_type_variants_are_distinct() {
    assert_ne!(AssetType::Css, AssetType::Html);
    assert_ne!(AssetType::Image, AssetType::Font);
    assert_ne!(AssetType::JavaScript, AssetType::Css);
}

#[test]
fn asset_type_equality() {
    assert_eq!(AssetType::Css, AssetType::Css);
    assert_eq!(AssetType::Html, AssetType::Html);
    assert_eq!(AssetType::Image, AssetType::Image);
    assert_eq!(AssetType::Font, AssetType::Font);
    assert_eq!(AssetType::JavaScript, AssetType::JavaScript);
}

#[test]
fn asset_definition_css_factory() {
    let asset = AssetDefinition::css(PathBuf::from("style.css"), "/css/style.css");
    assert_eq!(asset.asset_type(), AssetType::Css);
    assert_eq!(asset.destination(), "/css/style.css");
    assert!(asset.is_required());
}

#[test]
fn asset_definition_html_factory() {
    let asset = AssetDefinition::html(PathBuf::from("index.html"), "/index.html");
    assert_eq!(asset.asset_type(), AssetType::Html);
    assert_eq!(asset.destination(), "/index.html");
}

#[test]
fn asset_definition_image_factory() {
    let asset = AssetDefinition::image(PathBuf::from("logo.png"), "/img/logo.png");
    assert_eq!(asset.asset_type(), AssetType::Image);
}

#[test]
fn asset_definition_font_factory() {
    let asset = AssetDefinition::font(PathBuf::from("inter.woff2"), "/fonts/inter.woff2");
    assert_eq!(asset.asset_type(), AssetType::Font);
}

#[test]
fn asset_definition_javascript_factory() {
    let asset = AssetDefinition::javascript(PathBuf::from("app.js"), "/js/app.js");
    assert_eq!(asset.asset_type(), AssetType::JavaScript);
}

#[test]
fn asset_definition_js_alias() {
    let asset = AssetDefinition::js(PathBuf::from("main.js"), "/js/main.js");
    assert_eq!(asset.asset_type(), AssetType::JavaScript);
}

#[test]
fn asset_definition_source_path() {
    let asset = AssetDefinition::css(PathBuf::from("/absolute/style.css"), "/css/style.css");
    assert_eq!(asset.source(), PathBuf::from("/absolute/style.css"));
}

#[test]
fn asset_definition_builder_creates_required_by_default() {
    let asset =
        AssetDefinitionBuilder::new(PathBuf::from("test.css"), "/css/test.css", AssetType::Css)
            .build();
    assert!(asset.is_required());
}

#[test]
fn asset_definition_builder_optional() {
    let asset =
        AssetDefinitionBuilder::new(PathBuf::from("opt.css"), "/css/opt.css", AssetType::Css)
            .optional()
            .build();
    assert!(!asset.is_required());
}

#[test]
fn asset_definition_builder_via_static_method() {
    let asset = AssetDefinition::builder(
        PathBuf::from("built.js"),
        "/js/built.js",
        AssetType::JavaScript,
    )
    .build();
    assert_eq!(asset.asset_type(), AssetType::JavaScript);
    assert_eq!(asset.destination(), "/js/built.js");
}
