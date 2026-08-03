//! Tests for asset-type classification in `web assets list`.
//!
//! `determine_asset_type` decides the label every listed asset carries; the
//! favicon, logo-directory, extension, and fallback branches are distinct.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use systemprompt_cli::CliConfig;
use systemprompt_cli::web::assets::list::{AssetTypeFilter, ListArgs, execute_in_dir};

fn cfg() -> CliConfig {
    CliConfig::new().with_interactive(false)
}

fn types_by_path(dir: &Path, filter: AssetTypeFilter) -> HashMap<String, String> {
    let out = execute_in_dir(ListArgs { asset_type: filter }, &cfg(), dir).unwrap();
    let json = serde_json::to_value(out.artifact()).unwrap();
    json["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            (
                item["path"].as_str().unwrap().to_owned(),
                item["asset_type"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

fn seed_all_kinds(dir: &Path) {
    fs::create_dir_all(dir.join("logos")).unwrap();
    fs::write(dir.join("favicon.ico"), [0u8; 4]).unwrap();
    fs::write(dir.join("logos/mark.svg"), "<svg/>").unwrap();
    fs::write(dir.join("company-logo.png"), [0u8; 4]).unwrap();
    fs::write(dir.join("theme.css"), "body{}").unwrap();
    fs::write(dir.join("body.woff2"), [0u8; 4]).unwrap();
    fs::write(dir.join("hero.jpeg"), [0u8; 4]).unwrap();
    fs::write(dir.join("notes.txt"), "plain").unwrap();
}

#[test]
fn each_asset_kind_is_classified_by_its_own_rule() {
    let dir = tempfile::tempdir().unwrap();
    seed_all_kinds(dir.path());

    let types = types_by_path(dir.path(), AssetTypeFilter::All);

    assert_eq!(
        types.get("favicon.ico").map(String::as_str),
        Some("favicon")
    );
    assert_eq!(
        types.get("logos/mark.svg").map(String::as_str),
        Some("logo")
    );
    assert_eq!(
        types.get("company-logo.png").map(String::as_str),
        Some("logo")
    );
    assert_eq!(types.get("theme.css").map(String::as_str), Some("css"));
    assert_eq!(types.get("body.woff2").map(String::as_str), Some("font"));
    assert_eq!(types.get("hero.jpeg").map(String::as_str), Some("image"));
    assert_eq!(types.get("notes.txt").map(String::as_str), Some("other"));
}

#[test]
fn the_type_filter_restricts_the_listing_to_one_kind() {
    let dir = tempfile::tempdir().unwrap();
    seed_all_kinds(dir.path());

    let css = types_by_path(dir.path(), AssetTypeFilter::Css);
    assert!(css.values().all(|t| t == "css"), "{css:?}");
    assert!(css.contains_key("theme.css"));

    let fonts = types_by_path(dir.path(), AssetTypeFilter::Font);
    assert!(fonts.values().all(|t| t == "font"), "{fonts:?}");
    assert!(fonts.contains_key("body.woff2"));

    // The image filter admits every pictorial asset, so favicons and logos
    // ride along under their own labels.
    let images = types_by_path(dir.path(), AssetTypeFilter::Image);
    assert!(images.contains_key("hero.jpeg"), "{images:?}");
    assert!(!images.contains_key("theme.css"), "{images:?}");
    assert!(!images.contains_key("body.woff2"), "{images:?}");
    assert!(!images.contains_key("notes.txt"), "{images:?}");
}

#[test]
fn a_favicon_wins_over_its_extension_and_a_logo_over_its_type() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("favicon.png"), [0u8; 4]).unwrap();
    fs::write(dir.path().join("logo.css"), "body{}").unwrap();

    let types = types_by_path(dir.path(), AssetTypeFilter::All);
    assert_eq!(
        types.get("favicon.png").map(String::as_str),
        Some("favicon")
    );
    assert_eq!(types.get("logo.css").map(String::as_str), Some("logo"));
}
