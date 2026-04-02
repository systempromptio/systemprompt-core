use std::sync::Arc;

use systemprompt_extension::{Extension, ExtensionMetadata, ExtensionRegistry};

struct FakeExt {
    meta: ExtensionMetadata,
    deps: Vec<&'static str>,
    priority_val: u32,
    required: bool,
}

impl FakeExt {
    fn new(id: &'static str, name: &'static str) -> Self {
        Self {
            meta: ExtensionMetadata {
                id,
                name,
                version: "1.0.0",
            },
            deps: vec![],
            priority_val: 100,
            required: false,
        }
    }

    fn with_priority(mut self, p: u32) -> Self {
        self.priority_val = p;
        self
    }

    fn with_deps(mut self, deps: Vec<&'static str>) -> Self {
        self.deps = deps;
        self
    }

    fn required(mut self) -> Self {
        self.required = true;
        self
    }
}

impl Extension for FakeExt {
    fn metadata(&self) -> ExtensionMetadata {
        self.meta
    }

    fn dependencies(&self) -> Vec<&'static str> {
        self.deps.clone()
    }

    fn priority(&self) -> u32 {
        self.priority_val
    }

    fn is_required(&self) -> bool {
        self.required
    }
}

fn arc_ext(id: &'static str, name: &'static str) -> Arc<dyn Extension> {
    Arc::new(FakeExt::new(id, name))
}

#[test]
fn registry_new_is_empty() {
    let registry = ExtensionRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
}

#[test]
fn registry_default_is_empty() {
    let registry = ExtensionRegistry::default();
    assert!(registry.is_empty());
}

#[test]
fn registry_register_single() {
    let mut registry = ExtensionRegistry::new();
    let result = registry.register(arc_ext("ext-a", "Extension A"));
    assert!(result.is_ok());
    assert_eq!(registry.len(), 1);
}

#[test]
fn registry_register_multiple() {
    let mut registry = ExtensionRegistry::new();
    registry
        .register(arc_ext("ext-a", "Extension A"))
        .expect("register a");
    registry
        .register(arc_ext("ext-b", "Extension B"))
        .expect("register b");
    assert_eq!(registry.len(), 2);
}

#[test]
fn registry_register_duplicate_fails() {
    let mut registry = ExtensionRegistry::new();
    registry
        .register(arc_ext("dup", "Dup Extension"))
        .expect("first register");
    let result = registry.register(arc_ext("dup", "Dup Extension 2"));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("dup"));
}

#[test]
fn registry_has_returns_true_for_registered() {
    let mut registry = ExtensionRegistry::new();
    registry
        .register(arc_ext("find-me", "Find Me"))
        .expect("register");
    assert!(registry.has("find-me"));
}

#[test]
fn registry_has_returns_false_for_missing() {
    let registry = ExtensionRegistry::new();
    assert!(!registry.has("nonexistent"));
}

#[test]
fn registry_get_returns_extension() {
    let mut registry = ExtensionRegistry::new();
    registry
        .register(arc_ext("get-me", "Get Me"))
        .expect("register");
    let ext = registry.get("get-me");
    assert!(ext.is_some());
    assert_eq!(ext.unwrap().id(), "get-me");
}

#[test]
fn registry_get_returns_none_for_missing() {
    let registry = ExtensionRegistry::new();
    assert!(registry.get("missing").is_none());
}

#[test]
fn registry_ids_returns_all_ids() {
    let mut registry = ExtensionRegistry::new();
    registry
        .register(arc_ext("id-a", "A"))
        .expect("register a");
    registry
        .register(arc_ext("id-b", "B"))
        .expect("register b");
    let ids = registry.ids();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&"id-a"));
    assert!(ids.contains(&"id-b"));
}

#[test]
fn registry_extensions_sorted_by_priority() {
    let mut registry = ExtensionRegistry::new();
    registry
        .register(Arc::new(FakeExt::new("high", "High Priority").with_priority(200)))
        .expect("register high");
    registry
        .register(Arc::new(FakeExt::new("low", "Low Priority").with_priority(10)))
        .expect("register low");
    let exts = registry.extensions();
    assert_eq!(exts[0].id(), "low");
    assert_eq!(exts[1].id(), "high");
}

#[test]
fn registry_merge_extensions() {
    let mut registry = ExtensionRegistry::new();
    let extensions = vec![
        arc_ext("merge-a", "Merge A"),
        arc_ext("merge-b", "Merge B"),
    ];
    let result = registry.merge(extensions);
    assert!(result.is_ok());
    assert_eq!(registry.len(), 2);
}

#[test]
fn registry_merge_duplicate_fails() {
    let mut registry = ExtensionRegistry::new();
    registry
        .register(arc_ext("existing", "Existing"))
        .expect("register");
    let extensions = vec![arc_ext("existing", "Existing Again")];
    let result = registry.merge(extensions);
    assert!(result.is_err());
}

#[test]
fn registry_validate_no_deps_succeeds() {
    let mut registry = ExtensionRegistry::new();
    registry
        .register(arc_ext("no-deps", "No Deps"))
        .expect("register");
    assert!(registry.validate().is_ok());
}

#[test]
fn registry_validate_missing_dep_fails() {
    let mut registry = ExtensionRegistry::new();
    let ext = Arc::new(FakeExt::new("needs-dep", "Needs Dep").with_deps(vec!["missing-dep"]));
    registry.register(ext).expect("register");
    let result = registry.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("missing-dep"));
}

#[test]
fn registry_validate_satisfied_deps_succeeds() {
    let mut registry = ExtensionRegistry::new();
    registry
        .register(arc_ext("base", "Base"))
        .expect("register base");
    let dependent = Arc::new(FakeExt::new("child", "Child").with_deps(vec!["base"]));
    registry.register(dependent).expect("register child");
    assert!(registry.validate().is_ok());
}

#[test]
fn registry_enabled_extensions_filters_disabled() {
    let mut registry = ExtensionRegistry::new();
    registry
        .register(arc_ext("keep", "Keep"))
        .expect("register keep");
    registry
        .register(arc_ext("remove", "Remove"))
        .expect("register remove");
    let enabled = registry.enabled_extensions(&["remove".to_string()]);
    assert_eq!(enabled.len(), 1);
    assert_eq!(enabled[0].id(), "keep");
}

#[test]
fn registry_enabled_extensions_cannot_disable_required() {
    let mut registry = ExtensionRegistry::new();
    let required_ext = Arc::new(FakeExt::new("core", "Core").required());
    registry.register(required_ext).expect("register core");
    let enabled = registry.enabled_extensions(&["core".to_string()]);
    assert_eq!(enabled.len(), 1);
    assert_eq!(enabled[0].id(), "core");
}

#[test]
fn registry_debug_format() {
    let registry = ExtensionRegistry::new();
    let debug = format!("{registry:?}");
    assert!(debug.contains("ExtensionRegistry"));
}
