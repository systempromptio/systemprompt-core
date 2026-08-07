//! `parse_sha256sums` reads the digest the release's signed SHA256SUMS
//! advertises for one asset: two-space and binary-mode (`*`) entries both
//! resolve, an absent asset is `None`, and a prefix match must not satisfy a
//! different asset's lookup.

use systemprompt_api::routes::gateway::bridge_release::parse_sha256sums;

const SUMS: &str = "\
e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  systemprompt-internal-bridge-macos.zip
5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03 *systemprompt-internal-bridge-windows.exe
";

#[test]
fn reads_a_two_space_entry() {
    assert_eq!(
        parse_sha256sums(SUMS, "systemprompt-internal-bridge-macos.zip"),
        Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_owned())
    );
}

#[test]
fn reads_a_binary_mode_entry() {
    assert_eq!(
        parse_sha256sums(SUMS, "systemprompt-internal-bridge-windows.exe"),
        Some("5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03".to_owned())
    );
}

#[test]
fn a_missing_asset_is_none() {
    assert_eq!(parse_sha256sums(SUMS, "not-published.tar.gz"), None);
}

#[test]
fn names_must_match_exactly() {
    assert_eq!(parse_sha256sums(SUMS, "systemprompt-internal-bridge"), None);
}
