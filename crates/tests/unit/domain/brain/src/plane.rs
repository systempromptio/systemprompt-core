use std::collections::BTreeMap;
use std::path::Path;

use systemprompt_brain::models::PlaneRow;
use systemprompt_brain::plane::{BrainPlane, DeclaredBrain, PlaneRows, declared_hash_from_rows};
use systemprompt_extension::prelude::{ConfigPlane, ConfigPlaneError, PlaneSnapshot};

pub(crate) const TOPICS: &str = "topics:\n  - id: ops-TAG\n    name: Operations\n  - id: \
                                 billing-TAG\n    name: Billing\n";
pub(crate) const FILTERS: &str = "filters:\n  - name: documents\n    pathways: []\n";
pub(crate) const IMPORTANCE: &str = "base:\n  confirmed_by_owner: 0.25\n";
pub(crate) const RETRIEVAL: &str = "rrf_k: 50\nmax_facts: 8\n";
pub(crate) const KIND_OWNER: &str = "kind: owner-TAG\nsubject: service\nfields:\n  owner:\n    \
                                     type: text\n    required: true\ntopics: [ops-TAG]\n\
                                     statement: \"{subject} is owned by {owner}\"\nidentity: \
                                     [subject]\n";
pub(crate) const KIND_PRICE: &str = "kind: price-TAG\nsubject: product\nfields:\n  amount:\n    \
                                     type: number\ntopics: [billing-TAG]\nstatement: \"{subject} \
                                     costs {amount}\"\nidentity: [subject]\n";
const INTEREST: &str = "id: churn-TAG\nname: Churn\nowner: user-TAG\nquestion: Who is \
                        leaving?\n";
pub(crate) const WORKFLOW: &str = "id: support-TAG\nname: Support\nkinds: [owner-TAG]\n";

pub(crate) fn write(root: &Path, relative: &str, body: &str, tag: &str) {
    let path = root.join("brain").join(relative);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, body.replace("TAG", tag)).expect("write");
}

pub(crate) fn declare_without_interest(root: &Path, tag: &str) {
    write(root, "topics.yaml", TOPICS, tag);
    write(root, "filters.yaml", FILTERS, tag);
    write(root, "importance.yaml", IMPORTANCE, tag);
    write(root, "retrieval.yaml", RETRIEVAL, tag);
    write(root, "kinds/owner.yaml", KIND_OWNER, tag);
    write(root, "kinds/price.yaml", KIND_PRICE, tag);
    write(root, "profiles/support.yaml", WORKFLOW, tag);
}

fn declare(root: &Path) {
    declare_without_interest(root, "x");
    write(root, "interests/churn.yaml", INTEREST, "x");
}

fn entries(root: &Path) -> PlaneSnapshot {
    BrainPlane
        .load_declared(root)
        .expect("load")
        .expect("declared")
        .entries
}

fn invalid_message(root: &Path) -> String {
    match BrainPlane.load_declared(root) {
        Err(ConfigPlaneError::Invalid { message, .. }) => message,
        other => panic!("expected an invalid declaration, got {other:?}"),
    }
}

#[test]
fn absent_directory_is_undeclared() {
    let root = tempfile::tempdir().expect("tempdir");
    assert!(
        BrainPlane
            .load_declared(root.path())
            .expect("load")
            .is_none()
    );
}

#[test]
fn declared_tree_projects_every_entity() {
    let root = tempfile::tempdir().expect("tempdir");
    declare(root.path());
    let keys: Vec<String> = entries(root.path()).into_keys().collect();
    assert_eq!(
        keys,
        [
            "doc/filters",
            "doc/importance",
            "doc/retrieval",
            "interest/churn-x",
            "kind/owner-x",
            "kind/price-x",
            "topic/billing-x",
            "topic/ops-x",
            "workflow/support-x",
        ]
    );
}

#[test]
fn sources_name_the_declaring_file() {
    let root = tempfile::tempdir().expect("tempdir");
    declare(root.path());
    let sources = BrainPlane
        .sources(root.path())
        .expect("sources")
        .expect("declared");
    assert_eq!(
        sources["kind/price-x"].source.to_string(),
        "code:brain/kinds/price.yaml"
    );
    assert_eq!(
        sources["topic/ops-x"].source.to_string(),
        "code:brain/topics.yaml"
    );
}

#[test]
fn unknown_topic_in_a_kind_is_refused() {
    let root = tempfile::tempdir().expect("tempdir");
    declare(root.path());
    write(
        root.path(),
        "kinds/price.yaml",
        &KIND_PRICE.replace("[billing-TAG]", "[nowhere]"),
        "x",
    );
    assert!(invalid_message(root.path()).contains("nowhere"));
}

#[test]
fn duplicate_key_across_files_is_refused() {
    let root = tempfile::tempdir().expect("tempdir");
    declare(root.path());
    write(root.path(), "kinds/owner-again.yaml", KIND_OWNER, "x");
    assert!(invalid_message(root.path()).contains("owner-x"));
}

#[test]
fn rendered_declarations_load_back_to_the_same_entities() {
    let root = tempfile::tempdir().expect("tempdir");
    declare(root.path());
    let before = entries(root.path());
    let rendered = BrainPlane.render_declarations(&before).expect("render");
    let files: BTreeMap<String, String> = serde_yaml::from_str(&rendered).expect("file set");
    let copy = tempfile::tempdir().expect("tempdir");
    for (relative, body) in &files {
        write(copy.path(), relative, body, "x");
    }
    assert_eq!(entries(copy.path()), before);
}

fn rows_of(snapshot: &PlaneSnapshot) -> PlaneRows {
    let brain = DeclaredBrain::from_entities(snapshot).expect("typed");
    let mut rows = PlaneRows::default();
    for (key, value) in brain.entities().expect("entities") {
        let (kind, id) = key.split_once('/').expect("key");
        let row = PlaneRow {
            id: id.to_owned(),
            body: serde_json::from_str(&value).expect("json"),
            source: "file".to_owned(),
            updated_at: chrono::Utc::now(),
        };
        match kind {
            "kind" => rows.kinds.push(row),
            "topic" => rows.topics.push(row),
            "interest" => rows.interests.push(row),
            "workflow" => rows.workflows.push(row),
            _ => rows.docs.push(row),
        }
    }
    rows
}

#[test]
fn row_hash_equals_file_hash_and_tracks_changes() {
    let root = tempfile::tempdir().expect("tempdir");
    declare(root.path());
    let declared = BrainPlane
        .load_declared(root.path())
        .expect("load")
        .expect("declared");
    let mut rows = rows_of(&declared.entries);
    assert_eq!(declared_hash_from_rows(&rows).expect("hash"), declared.hash);
    let retrieval = rows
        .docs
        .iter_mut()
        .find(|row| row.id == "retrieval")
        .expect("retrieval row");
    retrieval.body["max_facts"] = serde_json::json!(9);
    assert_ne!(declared_hash_from_rows(&rows).expect("hash"), declared.hash);
}
