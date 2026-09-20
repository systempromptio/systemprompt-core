use systemprompt_loader::bundle::{owning_bundle_hashes, sources_provenance};

struct FixtureDir(std::path::PathBuf);

impl Drop for FixtureDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn base_tree_provenance_changes_with_declarations_and_recovers_after_removal() {
    let bootstrap = systemprompt_test_fixtures::ensure_test_bootstrap();
    let unique = format!(
        "provenance_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos()
    );
    let skill = bootstrap.services_path.join("skills").join(&unique);
    std::fs::create_dir_all(&skill).expect("unique local skill directory");
    let fixture = FixtureDir(skill);
    std::fs::write(
        fixture.0.join("config.yaml"),
        format!("id: {unique}\nname: Provenance fixture\ndescription: first\n"),
    )
    .expect("local declaration");

    let first = sources_provenance();
    let first_hash = first
        .base_tree_hash
        .expect("base tree hash after declaration");
    assert!(
        !owning_bundle_hashes().contains_key(&unique),
        "an unbundled local declaration must not acquire bundle provenance"
    );

    std::fs::write(fixture.0.join("index.md"), "# Trusted local instructions\n")
        .expect("declared content");
    let second = sources_provenance();
    let second_hash = second.base_tree_hash.expect("updated base tree hash");
    assert_ne!(first_hash, second_hash, "content surface changed");

    std::fs::remove_file(fixture.0.join("index.md")).expect("remove declared content");
    let restored = sources_provenance();
    assert_eq!(
        restored.base_tree_hash.as_deref(),
        Some(first_hash.as_str())
    );
}
