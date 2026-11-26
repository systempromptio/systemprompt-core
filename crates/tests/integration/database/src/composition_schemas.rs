//! Every booted composition's schema installs on a fresh database.
//!
//! `scripts/composition-members.txt` lists the member ids of each booted
//! composition (CI compares it with the `members` example of the facade built
//! with that line's features). The schema-bearing ones among them, taken from
//! the core members in reference order, are installed through the one install
//! path on an empty database, where the declarative schemas materialise the
//! state and each migration is stamped: a schema statement that names a table
//! owned by a member the composition leaves out fails the install. A second
//! install of the same members changes nothing in the catalog, and the
//! trace-facts rollup refreshes on the installed schema whichever ledger
//! owners the composition admits.

use systemprompt_database::{Database, schema_catalog_digest};
use systemprompt_test_fixtures::{DisposableDb, core_members, repo_path};

use crate::composition::install;

fn compositions() -> Vec<(String, Vec<String>)> {
    let path = repo_path("scripts/composition-members.txt");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let (name, ids) = line
                .split_once(char::is_whitespace)
                .unwrap_or_else(|| panic!("a composition row names its members: {line}"));
            (
                name.to_owned(),
                ids.trim().split(',').map(str::to_owned).collect(),
            )
        })
        .collect()
}

#[test]
fn the_member_list_names_the_runtime_and_reference_compositions() {
    let names: Vec<String> = compositions().into_iter().map(|(name, _)| name).collect();
    for expected in ["reference", "runtime"] {
        assert!(names.iter().any(|name| name == expected), "{names:?}");
    }
}

#[test]
fn the_reference_composition_links_every_schema_bearing_core_member() {
    let (_, reference) = compositions()
        .into_iter()
        .find(|(name, _)| name == "reference")
        .expect("the reference composition is listed");
    let missing: Vec<&str> = core_members()
        .iter()
        .map(|ext| ext.metadata().id)
        .filter(|id| !reference.iter().any(|member| member == id))
        .collect();
    assert!(missing.is_empty(), "{missing:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_booted_composition_installs_its_schema_on_a_fresh_database() {
    let mut refused = Vec::new();
    for (name, ids) in compositions() {
        let database =
            DisposableDb::empty(&format!("composition_{}", name.replace('-', "_"))).await;
        let db = Database::new_postgres(database.url())
            .await
            .unwrap_or_else(|e| panic!("{name}: connect: {e}"));
        let outcome = install_twice(&name, &ids, &db).await;
        db.pool().as_ref().clone().close().await;
        drop(db);
        database.drop_now().await;
        if let Err(error) = outcome {
            refused.push(format!("{name}: {error}"));
        }
    }
    assert!(refused.is_empty(), "{refused:#?}");
}

fn members_of(ids: &[String]) -> Vec<Box<dyn systemprompt_extension::Extension>> {
    core_members()
        .into_iter()
        .filter(|ext| ids.iter().any(|id| id == ext.metadata().id))
        .collect()
}

async fn install_twice(name: &str, ids: &[String], db: &Database) -> Result<(), String> {
    install(members_of(ids), db)
        .await
        .map_err(|error| format!("fresh install: {error:?}"))?;
    let first = schema_catalog_digest(db)
        .await
        .map_err(|error| format!("digest: {error:?}"))?;
    let report = install(members_of(ids), db)
        .await
        .map_err(|error| format!("second install: {error:?}"))?;
    let second = schema_catalog_digest(db)
        .await
        .map_err(|error| format!("digest: {error:?}"))?;
    if first != second {
        return Err(format!("{name}: the second install changed the catalog"));
    }
    if !report.foreign_key_drift.is_empty() || !report.held_back.is_empty() {
        return Err(format!("{name}: the second install reported {report:?}"));
    }
    sqlx::query_scalar::<_, i64>("SELECT refresh_trace_facts(ARRAY['composition-probe'])")
        .fetch_one(db.pool().as_ref())
        .await
        .map_err(|error| format!("{name}: refresh_trace_facts: {error:?}"))?;
    Ok(())
}
