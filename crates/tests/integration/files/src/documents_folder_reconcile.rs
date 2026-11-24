//! Folder entity reconcile over a 4,000-folder tree: every live folder
//! reachable from the root gets its `access_control_entities` row, a live
//! folder under a trashed one does not, and a row an operator already set is
//! left as it was. The reconcile reads the tree in one statement and inserts
//! the missing rows in bounded batches.

use std::collections::HashSet;
use std::time::Instant;

use systemprompt_governance::authz::EntityKind;
use systemprompt_test_fixtures::test_db_pool;
use uuid::Uuid;

use crate::documents_service_fixture::{GroupSource, allow, harness, unique};

const FOLDERS: usize = 4_000;
const FANOUT: usize = 10;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reconcile_registers_every_reachable_live_folder_of_a_large_tree() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let db = test_db_pool().await;
    let raw = db.write_pool();
    let owner = unique("reconcile-owner");

    let ids: Vec<Uuid> = (0..FOLDERS).map(|_| Uuid::new_v4()).collect();
    let parents: Vec<Option<Uuid>> = (0..FOLDERS)
        .map(|i| (i > 0).then(|| ids[(i - 1) / FANOUT]))
        .collect();
    let names: Vec<String> = (0..FOLDERS).map(|i| format!("{owner}-{i}")).collect();
    sqlx::query(
        "INSERT INTO document_folders (id, parent_id, name, owner_user_id, created_by)
         SELECT id, parent_id, name, $4, $4
         FROM unnest($1::uuid[], $2::uuid[], $3::text[]) AS t(id, parent_id, name)",
    )
    .bind(&ids)
    .bind(&parents)
    .bind(&names)
    .bind(&owner)
    .execute(raw.as_ref())
    .await
    .expect("seed the folder tree");

    let trashed = Uuid::new_v4();
    let orphan = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO document_folders (id, parent_id, name, owner_user_id, created_by, deleted_at)
         VALUES ($1, NULL, $3, $4, $4, now()), ($2, $1, $5, $4, $4, NULL)",
    )
    .bind(trashed)
    .bind(orphan)
    .bind(format!("{owner}-trashed"))
    .bind(&owner)
    .bind(format!("{owner}-under-trash"))
    .execute(raw.as_ref())
    .await
    .expect("seed a trashed subtree");

    let preset = ids[FOLDERS / 2].to_string();
    h.access_entities
        .upsert_entity(EntityKind::Folder, &preset, false, "operator")
        .await
        .expect("operator-set entity");

    let started = Instant::now();
    h.access
        .reconcile_folder_entities()
        .await
        .expect("reconcile");
    let elapsed = started.elapsed();
    println!("folder reconcile over {FOLDERS} folders: {elapsed:?}");

    let registered: HashSet<String> = h
        .access_entities
        .list_entities(EntityKind::Folder)
        .await
        .expect("entities")
        .into_iter()
        .map(|row| row.id)
        .collect();
    for id in &ids {
        assert!(
            registered.contains(&id.to_string()),
            "live folder {id} is registered"
        );
    }
    assert!(registered.contains(&Uuid::nil().to_string()), "root is registered");
    assert!(
        !registered.contains(&trashed.to_string()),
        "a trashed folder is not registered"
    );
    assert!(
        !registered.contains(&orphan.to_string()),
        "a live folder under a trashed one is not registered"
    );
    let kept = h
        .access_entities
        .get_entity(EntityKind::Folder, &preset)
        .await
        .expect("preset entity")
        .expect("present");
    assert!(!kept.default_included, "an operator-set row is left untouched");
    assert_eq!(kept.source, "operator");
}
