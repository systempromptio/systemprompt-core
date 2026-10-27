#[test]
fn bridge_layer_lint_passes_with_the_drive_module() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../..");
    let output = std::process::Command::new("bash")
        .arg(repo.join("scripts/lint-bridge-layers.sh"))
        .current_dir(&repo)
        .output()
        .expect("run lint-bridge-layers");
    assert!(
        output.status.success(),
        "lint-bridge-layers failed:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
