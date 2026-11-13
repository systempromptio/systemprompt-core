//! Tests for `ExtensionRegistry` query/filter methods in
//! `registry/queries.rs`: every collector walks only the enabled extensions,
//! and the schema install set is every linked schema bearer.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use systemprompt_extension::{
    AuthzPolicy, ContextKind, Extension, ExtensionContext, ExtensionMetadata, ExtensionRegistry,
    ExtensionRouter, LoaderError, SchemaDefinition,
};
use systemprompt_identifiers::ExtensionId;

use crate::stub_ctx::{StubCtx, registry_with};

struct CapExt {
    id: &'static str,
    schemas: bool,
    has_router: bool,
    router_builds: Arc<AtomicUsize>,
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

    fn required_storage_paths(&self) -> Vec<&'static str> {
        self.storage.clone()
    }

    fn is_required(&self) -> bool {
        self.required
    }
}

#[test]
fn the_install_set_is_every_linked_schema_bearer() {
    let mut with_schema = CapExt::new("schema-ext");
    with_schema.schemas = true;
    let registry = registry_with(vec![
        Arc::new(with_schema),
        Arc::new(CapExt::new("plain-ext")),
    ]);

    let schema_exts = registry
        .enabled_schema_extensions(ExtensionRegistry::INSTALL_EVERY_LINKED_SCHEMA)
        .expect("the install set disables nothing");
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
    let api = registry.api_routers(&ctx, &[]).expect("nothing disabled");
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
    let api = registry.api_routers(&ctx, &[]).expect("nothing disabled");

    assert_eq!(api.len(), 1);
    assert_eq!(builds.load(Ordering::SeqCst), 1);
}

#[test]
fn required_storage_paths_come_from_enabled_extensions_only() {
    let mut a = CapExt::new("a");
    a.storage = vec!["/data/a"];
    let mut b = CapExt::new("b");
    b.storage = vec!["/data/b1", "/data/b2"];
    let registry = registry_with(vec![
        Arc::new(a),
        Arc::new(b),
        Arc::new(CapExt::new("none")),
    ]);

    let mut paths = registry
        .all_required_storage_paths(&[])
        .expect("nothing disabled");
    paths.sort_unstable();
    assert_eq!(paths, vec!["/data/a", "/data/b1", "/data/b2"]);

    let mut enabled_paths = registry
        .all_required_storage_paths(&[ExtensionId::new("b")])
        .expect("b has no dependants");
    enabled_paths.sort_unstable();
    assert_eq!(enabled_paths, vec!["/data/a"]);
}

#[test]
fn capability_filters_empty_when_no_extension_declares() {
    let registry = registry_with(vec![Arc::new(CapExt::new("plain"))]);
    assert!(registry.config_extensions(&[]).expect("valid").is_empty());
    assert!(registry.asset_extensions(&[]).expect("valid").is_empty());
}

#[test]
fn enabled_extensions_refuses_to_disable_required() {
    let mut required = CapExt::new("core");
    required.required = true;
    let registry = registry_with(vec![Arc::new(required), Arc::new(CapExt::new("optional"))]);

    let disabled = [ExtensionId::new("core"), ExtensionId::new("optional")];
    let err = registry
        .enabled_extensions(&disabled)
        .err()
        .expect("required extension cannot be disabled");
    assert!(matches!(err, LoaderError::RequiredExtensionDisabled(id) if id == "core"));
    assert!(matches!(
        registry.all_required_storage_paths(&disabled),
        Err(LoaderError::RequiredExtensionDisabled(_))
    ));
    assert!(matches!(
        registry.all_metrics(&disabled),
        Err(LoaderError::RequiredExtensionDisabled(_))
    ));
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
