use std::collections::BTreeSet;
use std::path::PathBuf;

use systemprompt_extension::{AssetType, Extension};
use systemprompt_files::FilesExtension;

use super::{assets_root, declared, shipped_files};

#[test]
fn files_extension_declares_assets() {
    assert!(FilesExtension.declares_assets());
}

#[test]
fn required_assets_list_every_shipped_file() {
    let listed: BTreeSet<PathBuf> = declared()
        .iter()
        .map(|a| a.source().to_path_buf())
        .collect();
    assert_eq!(listed, shipped_files());
}

#[test]
fn destinations_mirror_the_assets_tree() {
    let root = assets_root();
    for asset in declared() {
        let relative = asset
            .source()
            .strip_prefix(&root)
            .expect("source under assets");
        let expected = format!("assets/documents/{}", relative.display());
        assert_eq!(asset.destination(), expected);
    }
}

#[test]
fn asset_types_follow_the_extension() {
    for asset in declared() {
        let ext = asset
            .source()
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default();
        let expected = match ext {
            "js" => AssetType::JavaScript,
            "css" => AssetType::Css,
            "wasm" => AssetType::Wasm,
            other => panic!("unexpected asset extension {other}"),
        };
        assert_eq!(asset.asset_type(), expected, "{}", asset.source().display());
    }
}

#[test]
fn wasm_asset_type_is_application_wasm() {
    assert_eq!(AssetType::Wasm.mime_type(), Some("application/wasm"));
    assert_eq!(AssetType::JavaScript.mime_type(), Some("text/javascript"));
    assert_eq!(AssetType::Image.mime_type(), None);
}

#[test]
fn loro_wasm_is_present_and_a_wasm_module() {
    let bytes = std::fs::read(assets_root().join("wasm/loro.wasm")).expect("loro.wasm");
    assert!(bytes.len() > 1024);
    assert_eq!(&bytes[..4], b"\0asm");
}

#[test]
fn vendored_loro_matches_the_pinned_crate() {
    let version = std::fs::read_to_string(assets_root().join("wasm/VERSION")).expect("VERSION");
    assert!(version.contains("version: 1.17.1"));
    assert!(version.contains("MIT"));
}
