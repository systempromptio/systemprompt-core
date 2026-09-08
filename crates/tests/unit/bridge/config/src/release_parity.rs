#[test]
fn bridge_tracks_the_core_release() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../..");
    let core: toml::Value =
        toml::from_str(&std::fs::read_to_string(root.join("Cargo.toml")).expect("core manifest"))
            .expect("core TOML");
    let bridge: toml::Value = toml::from_str(
        &std::fs::read_to_string(root.join("bin/bridge/Cargo.toml")).expect("bridge manifest"),
    )
    .expect("bridge TOML");
    assert_eq!(
        bridge["package"]["version"],
        core["workspace"]["package"]["version"]
    );
}
