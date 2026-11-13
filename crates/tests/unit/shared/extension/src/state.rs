//! Tests for typed extension state: `Extension::provide` run by
//! `ExtensionRegistry::build_state` in dependency order, `ExtensionState`
//! lookups, duplicate providers, declared requirements and the dependency
//! graph checks that run before any state is built.

use std::sync::{Arc, Mutex};

use systemprompt_extension::{
    Extension, ExtensionContext, ExtensionMetadata, ExtensionRegistry, ExtensionState,
    ExtensionStateError, LoaderError, StateBuilder, StateRequirement,
};

use crate::stub_ctx::{StubCtx, registry_with};

#[derive(Debug, PartialEq, Eq)]
struct Cache(u32);

#[derive(Debug, PartialEq, Eq)]
struct Derived(u32);

#[derive(Debug)]
struct NeverProvided;

#[derive(Clone, Copy)]
enum Provides {
    Nothing,
    Cache(u32),
    DerivedFromCache,
    Failure,
}

struct StateExt {
    id: &'static str,
    deps: Vec<&'static str>,
    priority: u32,
    provides: Provides,
    requires: Vec<StateRequirement>,
    log: Option<Arc<Mutex<Vec<&'static str>>>>,
}

impl StateExt {
    fn new(id: &'static str, provides: Provides) -> Self {
        Self {
            id,
            deps: vec![],
            priority: 100,
            provides,
            requires: vec![],
            log: None,
        }
    }

    fn with_deps(mut self, deps: Vec<&'static str>) -> Self {
        self.deps = deps;
        self
    }

    fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    fn with_requires(mut self, requires: Vec<StateRequirement>) -> Self {
        self.requires = requires;
        self
    }

    fn with_log(mut self, log: &Arc<Mutex<Vec<&'static str>>>) -> Self {
        self.log = Some(Arc::clone(log));
        self
    }
}

impl Extension for StateExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: self.id,
            version: "1.0.0",
        }
    }

    fn dependencies(&self) -> Vec<&'static str> {
        self.deps.clone()
    }

    fn priority(&self) -> u32 {
        self.priority
    }

    fn requires(&self) -> Vec<StateRequirement> {
        self.requires.clone()
    }

    fn provide(
        &self,
        _ctx: &dyn ExtensionContext,
        state: &mut StateBuilder<'_>,
    ) -> Result<(), ExtensionStateError> {
        if let Some(log) = &self.log {
            log.lock().expect("provide log").push(self.id);
        }
        assert_eq!(state.extension().as_str(), self.id);
        match self.provides {
            Provides::Nothing => Ok(()),
            Provides::Cache(value) => state.provide(Cache(value)),
            Provides::DerivedFromCache => {
                let cache = state.require::<Cache>()?;
                state.provide_arc(Arc::new(Derived(cache.0 * 2)))
            },
            Provides::Failure => Err(ExtensionStateError::Provider(Box::new(
                std::io::Error::other("cache warm-up failed"),
            ))),
        }
    }
}

fn build(exts: Vec<Arc<dyn Extension>>) -> Result<ExtensionState, LoaderError> {
    registry_with(exts).build_state(&StubCtx)
}

#[test]
fn default_state_is_empty() {
    let state = ExtensionState::default();
    assert!(state.is_empty());
    assert_eq!(state.len(), 0);
    assert!(state.get::<Cache>().is_none());
}

#[test]
fn provided_value_is_returned_by_get_and_require() {
    let state = build(vec![Arc::new(StateExt::new("cache", Provides::Cache(7)))])
        .expect("state builds");

    assert_eq!(state.len(), 1);
    assert_eq!(state.get::<Cache>().as_deref(), Some(&Cache(7)));
    assert_eq!(*state.require::<Cache>().expect("cache provided"), Cache(7));
    assert_eq!(
        state.provider_of::<Cache>().map(|id| id.as_str()),
        Some("cache")
    );
    assert!(state.contains(&StateRequirement::of::<Cache>()));
}

#[test]
fn clones_share_the_same_instance() {
    let state = build(vec![Arc::new(StateExt::new("cache", Provides::Cache(1)))])
        .expect("state builds");
    let clone = state.clone();

    let a = state.require::<Cache>().expect("cache");
    let b = clone.require::<Cache>().expect("cache");
    assert!(Arc::ptr_eq(&a, &b));
}

#[test]
fn require_of_missing_type_names_the_type() {
    let state = build(vec![Arc::new(StateExt::new("cache", Provides::Cache(1)))])
        .expect("state builds");

    let err = state.require::<NeverProvided>().expect_err("not provided");
    assert!(matches!(err, ExtensionStateError::Missing { type_name } if type_name.ends_with("NeverProvided")));
    assert!(err.to_string().contains("NeverProvided"), "got: {err}");
}

#[test]
fn duplicate_provider_names_both_extensions() {
    let err = build(vec![
        Arc::new(StateExt::new("first", Provides::Cache(1)).with_priority(10)),
        Arc::new(StateExt::new("second", Provides::Cache(2)).with_priority(20)),
    ])
    .expect_err("two providers of one type");

    let LoaderError::ProvideFailed { extension, source } = &err else {
        panic!("expected ProvideFailed, got {err:?}");
    };
    assert_eq!(extension.as_str(), "second");
    assert!(matches!(
        source,
        ExtensionStateError::DuplicateProvider { first, second, .. }
            if first.as_str() == "first" && second.as_str() == "second"
    ));
    let message = format!("{err}: {source}");
    assert!(message.contains("'first'"), "got: {message}");
    assert!(message.contains("'second'"), "got: {message}");
    assert!(message.contains("Cache"), "got: {message}");
}

#[test]
fn dependent_reads_state_its_dependency_provided() {
    let state = build(vec![
        Arc::new(
            StateExt::new("derived", Provides::DerivedFromCache)
                .with_deps(vec!["cache"])
                .with_priority(1),
        ),
        Arc::new(StateExt::new("cache", Provides::Cache(21)).with_priority(50)),
    ])
    .expect("dependency provides first");

    assert_eq!(state.get::<Derived>().as_deref(), Some(&Derived(42)));
    assert_eq!(
        state.provider_of::<Derived>().map(|id| id.as_str()),
        Some("derived")
    );
}

#[test]
fn reading_state_no_earlier_extension_provided_fails_provide() {
    let err = build(vec![Arc::new(StateExt::new(
        "derived",
        Provides::DerivedFromCache,
    ))])
    .expect_err("cache is never provided");

    assert!(matches!(
        &err,
        LoaderError::ProvideFailed { extension, source: ExtensionStateError::Missing { type_name } }
            if extension.as_str() == "derived" && type_name.ends_with("Cache")
    ));
}

#[test]
fn provider_failure_names_the_extension_and_keeps_the_cause() {
    let err = build(vec![Arc::new(StateExt::new("broken", Provides::Failure))])
        .expect_err("provide fails");

    assert!(matches!(&err, LoaderError::ProvideFailed { extension, .. } if extension.as_str() == "broken"));
    let source = std::error::Error::source(&err).expect("provide error is the source");
    let cause = source.source().expect("provider cause is kept");
    assert_eq!(cause.to_string(), "cache warm-up failed");
}

#[test]
fn unmet_requirement_fails_with_extension_and_type() {
    let err = build(vec![
        Arc::new(StateExt::new("cache", Provides::Cache(1))),
        Arc::new(
            StateExt::new("needy", Provides::Nothing)
                .with_requires(vec![StateRequirement::of::<NeverProvided>()]),
        ),
    ])
    .expect_err("requirement is unmet");

    assert!(matches!(
        &err,
        LoaderError::UnmetStateRequirement { extension, type_name }
            if extension.as_str() == "needy" && type_name.ends_with("NeverProvided")
    ));
    assert!(err.to_string().contains("NeverProvided"), "got: {err}");
}

#[test]
fn met_requirement_from_any_linked_extension_passes() {
    let state = build(vec![
        Arc::new(
            StateExt::new("needy", Provides::Nothing)
                .with_requires(vec![StateRequirement::of::<Cache>()]),
        ),
        Arc::new(StateExt::new("cache", Provides::Cache(3))),
    ])
    .expect("requirement is met");

    assert!(state.get::<Cache>().is_some());
}

#[test]
fn provide_order_is_dependency_first_then_priority_then_id() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let exts: Vec<Arc<dyn Extension>> = vec![
        Arc::new(StateExt::new("zeta", Provides::Nothing).with_log(&log)),
        Arc::new(
            StateExt::new("app", Provides::Nothing)
                .with_deps(vec!["base"])
                .with_priority(1)
                .with_log(&log),
        ),
        Arc::new(StateExt::new("alpha", Provides::Nothing).with_log(&log)),
        Arc::new(
            StateExt::new("base", Provides::Nothing)
                .with_priority(500)
                .with_log(&log),
        ),
    ];
    build(exts).expect("state builds");

    let order = log.lock().expect("provide log").clone();
    assert_eq!(order, vec!["base", "app", "alpha", "zeta"]);
}

#[test]
fn provide_order_is_stable_across_registration_order() {
    let run = |reverse: bool| {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut exts: Vec<Arc<dyn Extension>> = vec![
            Arc::new(StateExt::new("c", Provides::Nothing).with_log(&log)),
            Arc::new(StateExt::new("a", Provides::Nothing).with_log(&log)),
            Arc::new(
                StateExt::new("b", Provides::Nothing)
                    .with_deps(vec!["c"])
                    .with_log(&log),
            ),
        ];
        if reverse {
            exts.reverse();
        }
        build(exts).expect("state builds");
        log.lock().expect("provide log").clone()
    };

    assert_eq!(run(false), run(true));
    assert_eq!(run(false), vec!["a", "c", "b"]);
}

#[test]
fn dependency_cycle_is_refused_and_named() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut registry = ExtensionRegistry::new();
    let err = registry
        .merge(vec![
            Arc::new(
                StateExt::new("left", Provides::Nothing)
                    .with_deps(vec!["right"])
                    .with_log(&log),
            ),
            Arc::new(
                StateExt::new("right", Provides::Nothing)
                    .with_deps(vec!["left"])
                    .with_log(&log),
            ),
        ])
        .expect_err("two extensions depend on each other");

    let LoaderError::DependencyCycle { chain } = &err else {
        panic!("expected DependencyCycle, got {err:?}");
    };
    assert_eq!(chain, "left -> right -> left");
    assert!(log.lock().expect("provide log").is_empty());
}

#[test]
fn self_dependency_is_a_cycle() {
    let err = ExtensionRegistry::new()
        .register(Arc::new(
            StateExt::new("loop", Provides::Nothing).with_deps(vec!["loop"]),
        ))
        .expect_err("an extension cannot depend on itself");

    assert!(matches!(&err, LoaderError::DependencyCycle { chain } if chain == "loop -> loop"));
}

#[test]
fn merge_accepts_dependents_before_their_dependencies() {
    let mut registry = ExtensionRegistry::new();
    registry
        .merge(vec![
            Arc::new(StateExt::new("app", Provides::Nothing).with_deps(vec!["base"])),
            Arc::new(StateExt::new("base", Provides::Nothing)),
        ])
        .expect("merge orders the whole set at once");

    let ids: Vec<&str> = registry.extensions().iter().map(|e| e.id()).collect();
    assert_eq!(ids, vec!["base", "app"]);
}

#[test]
fn unknown_dependency_is_refused_before_any_provide() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut registry = ExtensionRegistry::new();
    let err = registry
        .register(Arc::new(
            StateExt::new("orphan", Provides::Cache(1))
                .with_deps(vec!["missing"])
                .with_log(&log),
        ))
        .expect_err("dependency is not linked");

    assert!(matches!(
        &err,
        LoaderError::MissingDependency { extension, dependency }
            if extension.as_str() == "orphan" && dependency.as_str() == "missing"
    ));
    assert!(log.lock().expect("provide log").is_empty());
}
