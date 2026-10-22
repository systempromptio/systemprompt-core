//! Migrate → load → resolve → compare with the 0.72.1 golden snapshots.
//!
//! Compared per requested id: whether it dispatches, `hidden`,
//! `advertised_to`, and per hop of the plain chain (and of every per-scope
//! chain the snapshot records, resolved with that scope attributed) the URL,
//! wire, credential, upstream model, price and weight. `info` and `served`
//! are not identity and are skipped.
//!
//! The allowlist of intended differences is exactly what was decided:
//! an alias route forwards its target model's `upstream_model` (0.72.1 sent
//! the alias string). `{install}` user agents and endpoint-scoped betas are
//! intended too but are not part of the snapshot schema. Every other
//! difference fails the test and is listed.

use serde_json::{Value, json};
use systemprompt_cli::commands::admin::config::migrate::{plan, write};
use systemprompt_identifiers::ScopeDimension;
use systemprompt_loader::ConfigLoader;
use systemprompt_manifest::services::catalog::{
    CatalogContext, MIGRATE_HINT, ModelRequest, ResolvedCatalog, ResolvedHop,
};
use systemprompt_models::attribution::{AttributionEntry, AttributionSource, RequestAttribution};
use systemprompt_models::providers::ClientFamily;

use crate::support::{Tree, fixtures_root, from_fixture, snapshot};

const FIXTURES: [&str; 3] = ["core-default", "dynapps", "astound"];
const PRICE_FIELDS: [&str; 5] = [
    "input_per_million",
    "output_per_million",
    "cache_read_per_million",
    "cache_write_per_million",
    "per_image_cents",
];

fn migrated(tree: &Tree) -> ResolvedCatalog {
    let plan = plan(&tree.services)
        .expect("migrate plans")
        .expect("legacy fixture needs a migration");
    write::apply(&tree.services, &plan).expect("migrate writes");
    ConfigLoader::load_from_path(&tree.root())
        .expect("migrated tree loads")
        .resolve_catalog(&CatalogContext::default())
        .expect("migrated catalog resolves")
}

fn hop_value(hop: &ResolvedHop) -> Value {
    let credential = hop.credential.platform_secret().map_or_else(
        || json!({ "source": "personal", "owner": hop.credential.id.as_str() }),
        |secret| json!({ "source": "platform", "secret": secret.as_str() }),
    );
    json!({
        "url": hop.url,
        "wire": hop.wire,
        "credential": credential,
        "upstream_model": hop.upstream_model,
        "pricing": hop.pricing,
        "weight": hop.weight,
    })
}

fn price_eq(a: &Value, b: &Value) -> bool {
    PRICE_FIELDS.iter().all(|field| {
        match (
            a.get(field).and_then(Value::as_f64),
            b.get(field).and_then(Value::as_f64),
        ) {
            (Some(x), Some(y)) => (x - y).abs() <= 1e-9 * x.abs().max(y.abs()).max(1.0),
            (None, None) => true,
            _ => false,
        }
    })
}

fn hop_diffs(golden: &Value, new: &Value) -> Vec<&'static str> {
    let mut diffs = Vec::new();
    for field in ["url", "wire", "credential", "upstream_model", "weight"] {
        if golden.get(field) != new.get(field) {
            diffs.push(field);
        }
    }
    if !golden
        .get("pricing")
        .zip(new.get("pricing"))
        .is_some_and(|(g, n)| price_eq(g, n))
    {
        diffs.push("pricing");
    }
    diffs
}

fn scope(dimension: &str, value: &str) -> RequestAttribution {
    RequestAttribution {
        entries: vec![AttributionEntry {
            dimension: ScopeDimension::new(dimension),
            value: value.to_owned(),
            source: AttributionSource::Header,
        }],
        api_key_id: None,
    }
}

fn chain(
    catalog: &ResolvedCatalog,
    id: &str,
    attribution: Option<&RequestAttribution>,
) -> Option<Vec<Value>> {
    let mut request = ModelRequest::new(id);
    if let Some(attribution) = attribution {
        request = request.with_attribution(attribution);
    }
    catalog
        .resolve(&request)
        .ok()
        .map(|t| t.chain.iter().map(hop_value).collect())
}

fn alias_target_upstream(catalog: &ResolvedCatalog, id: &str) -> Option<String> {
    let model = catalog.routes().iter().find_map(|r| r.aliases(id))?;
    catalog
        .enabled_hops(model.as_str())
        .next()
        .map(|h| h.upstream_model.clone())
}

fn compare_chain(
    catalog: &ResolvedCatalog,
    id: &str,
    label: &str,
    golden: &[Value],
    new: &[Value],
    out: &mut Vec<String>,
) {
    if golden.len() != new.len() {
        out.push(format!(
            "{id}{label}: chain has {} hops, 0.72.1 had {}",
            new.len(),
            golden.len()
        ));
    }
    for (index, (g, n)) in golden.iter().zip(new).enumerate() {
        let mut diffs = hop_diffs(g, n);
        let aliased = alias_target_upstream(catalog, id);
        if diffs == ["upstream_model"] && aliased.as_deref() == n["upstream_model"].as_str() {
            diffs.clear();
        }
        if !diffs.is_empty() {
            out.push(format!(
                "{id}{label} hop {index}: {} differ (0.72.1 {g}, migrated {n})",
                diffs.join(",")
            ));
        }
    }
}

fn advertised(catalog: &ResolvedCatalog, id: &str) -> Value {
    let model = catalog.model(id);
    let tags: Vec<&str> = ClientFamily::ALL
        .into_iter()
        .filter(|f| model.is_some_and(|m| m.is_advertised_to(*f)))
        .map(|f| {
            if f == ClientFamily::Claude {
                "anthropic"
            } else {
                f.as_tag()
            }
        })
        .collect();
    json!(tags)
}

fn diff_fixture(fixture: &str) -> Vec<String> {
    let tree = from_fixture(fixture);
    let catalog = migrated(&tree);
    let path = fixtures_root().join(fixture).join("golden/resolution.json");
    let golden: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("golden")).expect("golden json");
    let mut out = Vec::new();
    for (id, entry) in golden["models"].as_object().expect("models") {
        let new_chain = chain(&catalog, id, None);
        let Some(golden_chain) = entry.get("chain").and_then(Value::as_array) else {
            if new_chain.is_some() {
                out.push(format!(
                    "{id}: dispatches after migration; 0.72.1 refused it ({})",
                    entry["error"]
                ));
            }
            continue;
        };
        let Some(new_chain) = new_chain else {
            out.push(format!("{id}: no longer dispatches; 0.72.1 served it"));
            continue;
        };
        compare_chain(&catalog, id, "", golden_chain, &new_chain, &mut out);
        if catalog.model(id).is_some() && entry["advertised_to"] != advertised(&catalog, id) {
            out.push(format!(
                "{id}: advertised to {} (0.72.1 {})",
                advertised(&catalog, id),
                entry["advertised_to"]
            ));
        }
        if let Some(by_scope) = entry.get("by_scope") {
            let dimension = by_scope["dimension"].as_str().unwrap_or_default();
            for (value, golden_scoped) in by_scope["chains"].as_object().into_iter().flatten() {
                let attribution = scope(dimension, value);
                let scoped = chain(&catalog, id, Some(&attribution)).unwrap_or_default();
                let label = format!(" [{dimension}={value}]");
                compare_chain(
                    &catalog,
                    id,
                    &label,
                    golden_scoped.as_array().map_or(&[][..], Vec::as_slice),
                    &scoped,
                    &mut out,
                );
            }
        }
    }
    out
}

#[test]
fn migrated_fixtures_resolve_as_0_72_1_did() {
    let findings: Vec<String> = FIXTURES
        .iter()
        .flat_map(|fixture| {
            diff_fixture(fixture)
                .into_iter()
                .map(move |d| format!("{fixture}: {d}"))
        })
        .collect();
    assert!(
        findings.is_empty(),
        "differences outside the allowlist:\n{}",
        findings.join("\n")
    );
}

#[test]
fn second_migrate_writes_nothing() {
    for fixture in FIXTURES {
        let tree = from_fixture(fixture);
        migrated(&tree);
        let before = snapshot(&tree.services);
        assert!(
            plan(&tree.services).expect("plans").is_none(),
            "{fixture}: second run found a migration"
        );
        assert_eq!(
            before,
            snapshot(&tree.services),
            "{fixture}: second run changed files"
        );
    }
}

#[test]
fn legacy_tree_is_refused_with_the_migrate_hint() {
    for fixture in FIXTURES {
        let tree = from_fixture(fixture);
        let error =
            ConfigLoader::load_from_path(&tree.root()).expect_err("legacy tree must not load");
        assert!(
            error.to_string().contains(MIGRATE_HINT),
            "{fixture}: {error}"
        );
    }
}
