//! Tests for `ExtensionRegistry` query/filter methods in
//! `registry/queries.rs`: the capability filters (`schema_extensions`,
//! `enabled_schema_extensions`, `job_extensions`, ...) and the job
//! introspection manifest (`all_jobs`, `job_by_name`, `jobs_by_tag`).

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::{
    Extension, ExtensionContext, ExtensionMetadata, ExtensionRegistry, ExtensionRouter,
    SchemaDefinition,
};
use systemprompt_provider_contracts::{Job, JobContext, JobResult, ProviderResult};
use systemprompt_traits::{ConfigProvider, DatabaseHandle};

#[derive(Debug)]
struct StubJob {
    name: &'static str,
    tags: Vec<&'static str>,
}

#[async_trait]
impl Job for StubJob {
    fn name(&self) -> &'static str {
        self.name
    }

    fn schedule(&self) -> &'static str {
        "0 0 * * *"
    }

    fn tags(&self) -> Vec<&'static str> {
        self.tags.clone()
    }

    async fn execute(&self, _ctx: &JobContext) -> ProviderResult<JobResult> {
        Ok(JobResult::success())
    }
}

struct CapExt {
    id: &'static str,
    schemas: bool,
    has_router: bool,
    jobs: Vec<Arc<dyn Job>>,
    storage: Vec<&'static str>,
    required: bool,
}

impl CapExt {
    fn new(id: &'static str) -> Self {
        Self {
            id,
            schemas: false,
            has_router: false,
            jobs: Vec::new(),
            storage: Vec::new(),
            required: false,
        }
    }
}

impl Extension for CapExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "Cap",
            version: "1.0.0",
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        if self.schemas {
            vec![SchemaDefinition::new("t", "CREATE TABLE t (id TEXT)")]
        } else {
            vec![]
        }
    }

    fn router(&self, _ctx: &dyn ExtensionContext) -> Option<ExtensionRouter> {
        if self.has_router {
            Some(ExtensionRouter::new(axum::Router::new(), "/api/v1/cap"))
        } else {
            None
        }
    }

    fn jobs(&self) -> Vec<Arc<dyn Job>> {
        self.jobs.clone()
    }

    fn required_storage_paths(&self) -> Vec<&'static str> {
        self.storage.clone()
    }

    fn is_required(&self) -> bool {
        self.required
    }
}

struct StubCtx;

#[derive(Debug)]
struct StubConfig;

impl ConfigProvider for StubConfig {
    fn get(&self, _key: &str) -> Option<String> {
        None
    }
    fn database_url(&self) -> &str {
        "postgres://x/y"
    }
    fn system_path(&self) -> &str {
        "/tmp"
    }
    fn api_port(&self) -> u16 {
        0
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug)]
struct StubDb;

impl DatabaseHandle for StubDb {
    fn is_connected(&self) -> bool {
        true
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ExtensionContext for StubCtx {
    fn config(&self) -> Arc<dyn ConfigProvider> {
        Arc::new(StubConfig)
    }
    fn database(&self) -> Arc<dyn DatabaseHandle> {
        Arc::new(StubDb)
    }
    fn get_extension(&self, _id: &str) -> Option<Arc<dyn Extension>> {
        None
    }
}

fn registry_with(exts: Vec<Arc<dyn Extension>>) -> ExtensionRegistry {
    let mut registry = ExtensionRegistry::new();
    registry.merge(exts).expect("merge stub extensions");
    registry
}

#[test]
fn schema_extensions_filters_to_schema_bearers() {
    let mut with_schema = CapExt::new("schema-ext");
    with_schema.schemas = true;
    let registry = registry_with(vec![
        Arc::new(with_schema),
        Arc::new(CapExt::new("plain-ext")),
    ]);

    let schema_exts = registry.schema_extensions();
    assert_eq!(schema_exts.len(), 1);
    assert_eq!(schema_exts[0].id(), "schema-ext");
}

#[test]
fn enabled_schema_extensions_excludes_disabled_and_non_schema() {
    let mut a = CapExt::new("a");
    a.schemas = true;
    let mut b = CapExt::new("b");
    b.schemas = true;
    let registry = registry_with(vec![Arc::new(a), Arc::new(b), Arc::new(CapExt::new("c"))]);

    let enabled = registry.enabled_schema_extensions(&["a".to_string()]);
    let ids: Vec<_> = enabled.iter().map(|e| e.id()).collect();
    assert_eq!(
        ids,
        vec!["b"],
        "only schema-bearer b survives the disable of a"
    );
}

#[test]
fn api_extensions_filters_by_router_presence() {
    let mut routed = CapExt::new("routed");
    routed.has_router = true;
    let registry = registry_with(vec![Arc::new(routed), Arc::new(CapExt::new("unrouted"))]);

    let ctx = StubCtx;
    let api = registry.api_extensions(&ctx);
    assert_eq!(api.len(), 1);
    assert_eq!(api[0].id(), "routed");
}

#[test]
fn enabled_api_extensions_honours_disable_list() {
    let mut routed = CapExt::new("routed");
    routed.has_router = true;
    let registry = registry_with(vec![Arc::new(routed)]);

    let ctx = StubCtx;
    assert_eq!(registry.enabled_api_extensions(&ctx, &[]).len(), 1);
    assert!(
        registry
            .enabled_api_extensions(&ctx, &["routed".to_string()])
            .is_empty()
    );
}

#[test]
fn job_extensions_and_enabled_job_extensions() {
    let mut with_job = CapExt::new("worker");
    with_job.jobs = vec![Arc::new(StubJob {
        name: "cleanup",
        tags: vec![],
    })];
    let registry = registry_with(vec![Arc::new(with_job), Arc::new(CapExt::new("idle"))]);

    assert_eq!(registry.job_extensions().len(), 1);
    assert_eq!(registry.job_extensions()[0].id(), "worker");
    assert_eq!(registry.enabled_job_extensions(&[]).len(), 1);
    assert!(
        registry
            .enabled_job_extensions(&["worker".to_string()])
            .is_empty()
    );
}

#[test]
fn storage_extensions_and_all_required_storage_paths() {
    let mut a = CapExt::new("a");
    a.storage = vec!["/data/a"];
    let mut b = CapExt::new("b");
    b.storage = vec!["/data/b1", "/data/b2"];
    let registry = registry_with(vec![
        Arc::new(a),
        Arc::new(b),
        Arc::new(CapExt::new("none")),
    ]);

    assert_eq!(registry.storage_extensions().len(), 2);
    let mut paths = registry.all_required_storage_paths();
    paths.sort_unstable();
    assert_eq!(paths, vec!["/data/a", "/data/b1", "/data/b2"]);
}

#[test]
fn capability_filters_empty_when_no_extension_declares() {
    let registry = registry_with(vec![Arc::new(CapExt::new("plain"))]);
    assert!(registry.config_extensions().is_empty());
    assert!(registry.llm_provider_extensions().is_empty());
    assert!(registry.tool_provider_extensions().is_empty());
    assert!(registry.asset_extensions().is_empty());
}

#[test]
fn all_jobs_flattens_across_extensions() {
    let mut ext_a = CapExt::new("a");
    ext_a.jobs = vec![
        Arc::new(StubJob {
            name: "job-a1",
            tags: vec!["nightly"],
        }),
        Arc::new(StubJob {
            name: "job-a2",
            tags: vec!["hourly", "nightly"],
        }),
    ];
    let mut ext_b = CapExt::new("b");
    ext_b.jobs = vec![Arc::new(StubJob {
        name: "job-b1",
        tags: vec!["hourly"],
    })];
    let registry = registry_with(vec![Arc::new(ext_a), Arc::new(ext_b)]);

    let mut names: Vec<_> = registry.all_jobs().iter().map(|j| j.name()).collect();
    names.sort_unstable();
    assert_eq!(names, vec!["job-a1", "job-a2", "job-b1"]);
}

#[test]
fn job_by_name_finds_and_misses() {
    let mut ext = CapExt::new("worker");
    ext.jobs = vec![Arc::new(StubJob {
        name: "rotate-keys",
        tags: vec![],
    })];
    let registry = registry_with(vec![Arc::new(ext)]);

    let found = registry.job_by_name("rotate-keys").expect("job present");
    assert_eq!(found.name(), "rotate-keys");
    assert!(registry.job_by_name("does-not-exist").is_none());
}

#[test]
fn jobs_by_tag_filters_by_tag_membership() {
    let mut ext = CapExt::new("worker");
    ext.jobs = vec![
        Arc::new(StubJob {
            name: "nightly-only",
            tags: vec!["nightly"],
        }),
        Arc::new(StubJob {
            name: "both",
            tags: vec!["nightly", "hourly"],
        }),
        Arc::new(StubJob {
            name: "hourly-only",
            tags: vec!["hourly"],
        }),
    ];
    let registry = registry_with(vec![Arc::new(ext)]);

    let mut nightly: Vec<_> = registry
        .jobs_by_tag("nightly")
        .iter()
        .map(|j| j.name())
        .collect();
    nightly.sort_unstable();
    assert_eq!(nightly, vec!["both", "nightly-only"]);
    assert!(registry.jobs_by_tag("weekly").is_empty());
}

#[test]
fn enabled_extensions_keeps_required_even_when_disabled() {
    let mut required = CapExt::new("core");
    required.required = true;
    let registry = registry_with(vec![Arc::new(required), Arc::new(CapExt::new("optional"))]);

    let enabled = registry.enabled_extensions(&["core".to_string(), "optional".to_string()]);
    let ids: Vec<_> = enabled.iter().map(|e| e.id()).collect();
    assert_eq!(
        ids,
        vec!["core"],
        "required extension ignores the disable flag"
    );
}

// Guards against the `HashMap`-keyed lookups silently colliding across many
// extensions.
#[test]
fn ids_and_get_round_trip_over_many() {
    let exts: Vec<Arc<dyn Extension>> = (0..5)
        .map(|i| {
            let id: &'static str = Box::leak(format!("ext-{i}").into_boxed_str());
            Arc::new(CapExt::new(id)) as Arc<dyn Extension>
        })
        .collect();
    let registry = registry_with(exts);

    let mut by_id: HashMap<&str, &str> = HashMap::new();
    for id in registry.ids() {
        by_id.insert(id, registry.get(id).expect("present").id());
    }
    assert_eq!(by_id.len(), 5);
    for (k, v) in by_id {
        assert_eq!(k, v);
    }
}
