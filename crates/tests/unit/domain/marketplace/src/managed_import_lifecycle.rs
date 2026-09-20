use systemprompt_identifiers::UserId;
use systemprompt_marketplace::managed::{
    ManagedRepository, ResourceKind, SourceSpec, capture_skills,
};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_db_pool, seed_user_row};

fn write_skill(root: &std::path::Path, id: &str, instructions: &str) {
    let skill = root.join("skills").join(id);
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(
        skill.join("config.yaml"),
        format!(
            "id: {id}\nname: {id}\ndescription: imported {id}\nenabled: true\nfile: index.md\n"
        ),
    )
    .unwrap();
    std::fs::write(skill.join("index.md"), instructions).unwrap();
}

#[tokio::test]
async fn importing_captured_skills_retains_one_snapshot_and_immutable_files_per_skill() {
    let bootstrap = ensure_test_bootstrap();
    let db = fixture_db_pool(&bootstrap.database_url).await.unwrap();
    let owner = UserId::new(format!("import-owner-{}", uuid::Uuid::new_v4()));
    seed_user_row(&db, &owner, &format!("{owner}@managed.invalid"))
        .await
        .unwrap();
    let repository = ManagedRepository::new(&db).unwrap();
    let source = repository
        .register_source(&owner, "captured-authoring", &SourceSpec::Managed)
        .await
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    write_skill(root.path(), "alpha", "# Alpha\n");
    write_skill(root.path(), "beta", "# Beta\n");
    let captured = capture_skills(root.path(), &["beta".to_owned(), "alpha".to_owned()]).unwrap();

    let imported = repository
        .import_skills(&owner, &source, &captured, None)
        .await
        .unwrap();

    assert_eq!(imported.source_id, source);
    assert_eq!(
        imported
            .revisions
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["alpha", "beta"]
    );
    let provenance = repository
        .snapshot_provenance(&owner, &imported.snapshot_id)
        .await
        .unwrap();
    assert_eq!(provenance.source_kind, "managed");
    assert_eq!(provenance.commit, None);
    assert_eq!(provenance.tree_digest, captured.tree_digest().clone());

    let resources = repository.list_resources(&owner, 0).await.unwrap();
    assert_eq!(resources.items.len(), 2);
    for (key, revision) in &imported.revisions {
        let resource = resources
            .items
            .iter()
            .find(|item| item.resource_key == *key)
            .unwrap();
        assert_eq!(resource.kind, ResourceKind::Skill);
        assert_eq!(resource.source_id, source);
        assert_eq!(resource.revision_count, 1);
        assert_eq!(resource.latest_revision.as_ref(), Some(revision));
        let files = repository
            .get_revision_files(&owner, revision)
            .await
            .unwrap();
        assert!(files.0.contains_key("config.yaml"));
        assert_eq!(
            std::str::from_utf8(&files.0["index.md"].bytes).unwrap(),
            if key == "alpha" {
                "# Alpha\n"
            } else {
                "# Beta\n"
            }
        );
    }

    write_skill(root.path(), "alpha", "# Alpha revised\n");
    let revised = capture_skills(root.path(), &["alpha".to_owned()]).unwrap();
    let reimported = repository
        .import_skills(&owner, &source, &revised, None)
        .await
        .unwrap();
    let alpha_before = &imported.revisions["alpha"];
    let alpha_after = &reimported.revisions["alpha"];
    let resources = repository.list_resources(&owner, 0).await.unwrap();
    let alpha = resources
        .items
        .iter()
        .find(|item| item.resource_key == "alpha")
        .unwrap();

    assert_ne!(reimported.snapshot_id, imported.snapshot_id);
    assert_ne!(alpha_after, alpha_before);
    assert_eq!(resources.items.len(), 2);
    assert_eq!(alpha.revision_count, 2);
    assert_eq!(alpha.latest_revision.as_ref(), Some(alpha_after));
    assert_eq!(
        std::str::from_utf8(
            &repository
                .get_revision_files(&owner, alpha_before)
                .await
                .unwrap()
                .0["index.md"]
                .bytes,
        )
        .unwrap(),
        "# Alpha\n"
    );
    assert_eq!(
        std::str::from_utf8(
            &repository
                .get_revision_files(&owner, alpha_after)
                .await
                .unwrap()
                .0["index.md"]
                .bytes,
        )
        .unwrap(),
        "# Alpha revised\n"
    );

    let foreign_owner = UserId::new(format!("foreign-owner-{}", uuid::Uuid::new_v4()));
    seed_user_row(
        &db,
        &foreign_owner,
        &format!("{foreign_owner}@managed.invalid"),
    )
    .await
    .unwrap();
    let error = repository
        .import_skills(&foreign_owner, &source, &captured, None)
        .await
        .unwrap_err();

    assert!(error.to_string().contains("unavailable"), "{error}");
    assert!(
        repository
            .list_resources(&foreign_owner, 0)
            .await
            .unwrap()
            .items
            .is_empty()
    );
}
