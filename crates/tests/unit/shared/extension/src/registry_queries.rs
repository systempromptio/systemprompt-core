//! Tests for `ExtensionRegistry` query/filter methods in
//! `registry/queries.rs`: the capability filters (`schema_extensions`,
//! `enabled_schema_extensions`, `enabled_job_extensions`, `api_routers`, ...) and the job
//! introspection manifest (`all_jobs`, `job_by_name`, `jobs_by_tag`).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use systemprompt_extension::{
    AuthzPolicy, ContextKind, Extension, ExtensionContext, ExtensionMetadata, ExtensionRouter,
    LoaderError, SchemaDefinition,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::{Job, JobContext, JobResult, ProviderResult};

use crate::stub_ctx::{StubCtx, registry_with};

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
    router_builds: Arc<AtomicUsize>,
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
            router_builds: Arc::new(AtomicUsize::new(0)),
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
        self.router_builds.fetch_add(1, Ordering::SeqCst);
        if self.has_router {
            Some(ExtensionRouter::new(
                axum::Router::new(),
                "/api/v1/cap",
                AuthzPolicy::user(),
                ContextKind::User,
            ))
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

    let enabled = registry
        .enabled_schema_extensions(&[ExtensionId::new("a")])
        .expect("a has no dependants");
    let ids: Vec<_> = enabled.iter().map(|e| e.id()).collect();
    assert_eq!(
        ids,
        vec!["b"],
        "only schema-bearer b survives the disable of a"
    );
}

#[test]
fn api_routers_filters_by_router_presence() {
    let mut routed = CapExt::new("routed");
    routed.has_router = true;
    let registry = registry_with(vec![Arc::new(routed), Arc::new(CapExt::new("unrouted"))]);

    let ctx = StubCtx;
    let api = registry
        .api_routers(&ctx, &[])
        .expect("nothing disabled");
    assert_eq!(api.len(), 1);
    assert_eq!(api[0].0.id(), "routed");
    assert_eq!(api[0].1.base_path, "/api/v1/cap");
}

#[test]
fn api_routers_builds_each_router_exactly_once() {
    let mut routed = CapExt::new("routed");
    routed.has_router = true;
    let builds = Arc::clone(&routed.router_builds);
    let registry = registry_with(vec![Arc::new(routed)]);

    let ctx = StubCtx;
    let api = registry
        .api_routers(&ctx, &[])
        .expect("nothing disabled");

    assert_eq!(api.len(), 1);
    assert_eq!(builds.load(Ordering::SeqCst), 1);
}

#[test]
fn enabled_job_extensions_lists_only_job_bearers() {
    let mut with_job = CapExt::new("worker");
    with_job.jobs = vec![Arc::new(StubJob {
        name: "cleanup",
        tags: vec![],
    })];
    let registry = registry_with(vec![Arc::new(with_job), Arc::new(CapExt::new("idle"))]);

    assert_eq!(
        registry
            .enabled_job_extensions(&[])
            .expect("nothing disabled")
            .len(),
        1
    );
    assert!(
        registry
            .enabled_job_extensions(&[ExtensionId::new("worker")])
            .expect("worker has no dependants")
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
fn enabled_extensions_refuses_to_disable_required() {
    let mut required = CapExt::new("core");
    required.required = true;
    let registry = registry_with(vec![Arc::new(required), Arc::new(CapExt::new("optional"))]);

    let err = registry
        .enabled_extensions(&[ExtensionId::new("core"), ExtensionId::new("optional")])
        .err()
        .expect("required extension cannot be disabled");
    assert!(matches!(err, LoaderError::RequiredExtensionDisabled(id) if id == "core"));
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

#[tokio::test]
async fn a_context_without_coordination_refuses_liveness_and_leases() {
    let ctx = StubCtx;
    let liveness = ctx
        .instances()
        .is_alive(&systemprompt_identifiers::InstanceId::new("peer"))
        .await;
    assert!(matches!(
        liveness,
        Err(systemprompt_traits::CoordinationError::Unavailable { .. })
    ));
    let lease = ctx
        .leases()
        .claim(
            &systemprompt_identifiers::LeaseKey::new("lease"),
            std::time::Duration::from_secs(30),
        )
        .await;
    assert!(matches!(
        lease,
        Err(systemprompt_traits::CoordinationError::Unavailable { .. })
    ));
}

#[test]
fn an_unknown_secret_is_none_and_a_secret_never_prints_its_value() {
    assert!(
        StubCtx
            .secrets()
            .secret("sp_no_such_secret_registry")
            .is_none()
    );
    let secret = systemprompt_traits::ExtensionSecret::new("s3cr3t".to_owned());
    assert_eq!(secret.expose(), "s3cr3t");
    assert!(!format!("{secret:?}").contains("s3cr3t"));
}

#[test]
fn api_routers_skip_a_disabled_extension() {
    let mut kept = CapExt::new("kept");
    kept.has_router = true;
    let mut dropped = CapExt::new("dropped");
    dropped.has_router = true;
    let builds = Arc::clone(&dropped.router_builds);
    let registry = registry_with(vec![Arc::new(kept), Arc::new(dropped)]);

    let api = registry
        .api_routers(&StubCtx, &[ExtensionId::new("dropped")])
        .expect("the disabled set is valid");

    assert_eq!(api.len(), 1);
    assert_eq!(api[0].0.id(), "kept");
    assert_eq!(
        builds.load(Ordering::SeqCst),
        0,
        "a disabled extension's router must never be built"
    );
}
