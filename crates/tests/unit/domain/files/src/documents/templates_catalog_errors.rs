use std::path::Path;

use systemprompt_files::documents::templates::{TemplateCatalogError, catalog_dir, load_catalog};

fn root_with(entries: &[(&str, &str)]) -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("tempdir");
    let dir = catalog_dir(root.path());
    std::fs::create_dir_all(&dir).expect("dir");
    for (name, body) in entries {
        std::fs::write(dir.join(name), body).expect("write");
    }
    root
}

#[test]
fn catalog_dir_is_under_documents_templates() {
    assert_eq!(
        catalog_dir(Path::new("/srv")),
        Path::new("/srv/documents/templates")
    );
}

#[test]
fn entry_naming_a_missing_file_is_refused() {
    let root = root_with(&[(
        "a.yaml",
        "id: report\nformat: text\nname: Report\nfile: missing.txt\n",
    )]);
    let err = load_catalog(root.path()).expect_err("missing file");
    let TemplateCatalogError::MissingFile { path } = err else {
        panic!("expected MissingFile, got {err:?}");
    };
    assert!(path.ends_with("missing.txt"));
}

#[test]
fn entry_with_an_invalid_id_is_refused() {
    let root = root_with(&[
        ("body.txt", "hello\n"),
        (
            "a.yml",
            "id: \"\"\nformat: text\nname: Empty\nfile: body.txt\n",
        ),
    ]);
    let err = load_catalog(root.path()).expect_err("invalid id");
    assert!(
        matches!(err, TemplateCatalogError::InvalidId { .. }),
        "{err:?}"
    );
}

#[test]
fn non_yaml_files_and_directories_are_ignored() {
    let root = root_with(&[
        ("body.txt", "hello\n"),
        ("notes.md", "not a template"),
        (
            "ok.yaml",
            "id: ok\nformat: text\nname: Ok\nfile: body.txt\n",
        ),
    ]);
    std::fs::create_dir(catalog_dir(root.path()).join("nested.yaml")).expect("dir");
    let catalog = load_catalog(root.path()).expect("catalog");
    assert_eq!(catalog.len(), 1);
    assert_eq!(catalog[0].template.id.as_str(), "ok");
}

#[test]
fn catalog_path_that_is_a_file_is_empty() {
    let root = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(root.path().join("documents")).expect("dir");
    std::fs::write(catalog_dir(root.path()), "file").expect("write");
    assert!(load_catalog(root.path()).expect("catalog").is_empty());
}
