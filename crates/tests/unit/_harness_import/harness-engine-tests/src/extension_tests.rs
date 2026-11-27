//! The extension contract: resolution, enablement and header merging.

use std::sync::Arc;

use harness_engine::Endpoint;
use harness_engine::extension::{
    EnabledExtensions, EngineExtension, ExtensionMetadata, ExtensionSet, PromptContext,
    PromptSection, ResolveError, Resolved, add_headers, admitted, resolve,
};
use harness_ids::{ExtensionId, SessionId, extension_id};

fn id(value: &str) -> ExtensionId {
    ExtensionId::parse(value).unwrap()
}

fn meta(value: &str, priority: u32) -> ExtensionMetadata {
    ExtensionMetadata::new(id(value), "test").with_priority(priority)
}

fn candidate(metadata: ExtensionMetadata, tag: &'static str) -> Resolved<&'static str> {
    Resolved {
        metadata,
        value: tag,
    }
}

fn ids<T>(resolved: &[Resolved<T>]) -> Vec<&str> {
    resolved
        .iter()
        .map(|entry| entry.metadata.id.as_str())
        .collect()
}

#[test]
fn resolve_orders_by_priority_then_id() {
    let resolved = resolve([
        candidate(meta("beta", 10), "b"),
        candidate(meta("alpha", 10), "a"),
        candidate(meta("gamma", 50), "g"),
    ])
    .unwrap();
    assert_eq!(ids(&resolved), ["gamma", "alpha", "beta"]);
}

#[test]
fn a_higher_priority_shadows_the_same_id() {
    let resolved = resolve([
        candidate(meta("drive", 100), "stock"),
        candidate(meta("drive", 200), "white-label"),
    ])
    .unwrap();
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].value, "white-label");
}

#[test]
fn a_required_extension_is_not_shadowed_by_an_optional_one() {
    let resolved = resolve([
        candidate(meta("governance", 10).mark_required(), "required"),
        candidate(meta("governance", 900), "impostor"),
    ])
    .unwrap();
    assert_eq!(resolved[0].value, "required");
    assert!(resolved[0].metadata.required);
}

#[test]
fn an_unbroken_tie_on_one_id_is_refused() {
    let error = resolve([
        candidate(meta("drive", 100), "one"),
        candidate(meta("drive", 100), "two"),
    ])
    .unwrap_err();
    assert_eq!(
        error,
        ResolveError::Ambiguous {
            id: id("drive"),
            priority: 100
        }
    );
}

#[test]
fn a_tie_broken_by_a_higher_priority_resolves() {
    let resolved = resolve([
        candidate(meta("drive", 100), "one"),
        candidate(meta("drive", 100), "two"),
        candidate(meta("drive", 300), "three"),
    ])
    .unwrap();
    assert_eq!(resolved[0].value, "three");
}

#[test]
fn a_missing_dependency_is_refused() {
    let error = resolve([candidate(
        meta("slack", 1).depends_on(id("notifications")),
        "s",
    )])
    .unwrap_err();
    assert_eq!(
        error,
        ResolveError::MissingDependency {
            extension: id("slack"),
            dependency: id("notifications")
        }
    );
}

#[test]
fn a_dependency_cycle_is_refused_with_its_chain() {
    let error = resolve([
        candidate(meta("alpha", 1).depends_on(id("beta")), "a"),
        candidate(meta("beta", 1).depends_on(id("alpha")), "b"),
    ])
    .unwrap_err();
    assert_eq!(
        error,
        ResolveError::DependencyCycle {
            chain: vec![id("alpha"), id("beta"), id("alpha")]
        }
    );
    assert_eq!(
        error.to_string(),
        "extension dependencies form a cycle: alpha -> beta -> alpha"
    );
}

#[test]
fn a_required_extension_may_not_depend_on_an_optional_one() {
    let error = resolve([
        candidate(
            meta("governance", 1)
                .mark_required()
                .depends_on(id("drive")),
            "g",
        ),
        candidate(meta("drive", 1), "d"),
    ])
    .unwrap_err();
    assert!(matches!(error, ResolveError::RequiredOnOptional { .. }));
}

fn sample() -> Vec<Resolved<&'static str>> {
    resolve([
        candidate(meta("governance", 900).mark_required(), "g"),
        candidate(meta("notifications", 50), "n"),
        candidate(meta("slack", 40).depends_on(id("notifications")), "s"),
        candidate(meta("drive", 60), "d"),
    ])
    .unwrap()
}

#[test]
fn without_enablement_only_required_extensions_are_admitted() {
    let resolved = sample();
    let admitted: Vec<_> = admitted(&resolved, None)
        .iter()
        .map(|entry| entry.metadata.id.as_str())
        .collect();
    assert_eq!(admitted, ["governance"]);
}

#[test]
fn an_unlisted_extension_is_withheld() {
    let resolved = sample();
    let enabled: EnabledExtensions = std::iter::once(id("drive")).collect();
    let admitted: Vec<_> = admitted(&resolved, Some(&enabled))
        .iter()
        .map(|entry| entry.metadata.id.as_str())
        .collect();
    assert_eq!(admitted, ["governance", "drive"]);
}

#[test]
fn an_extension_whose_dependency_is_withheld_is_withheld() {
    let resolved = sample();
    let enabled: EnabledExtensions = std::iter::once(id("slack")).collect();
    let admitted: Vec<_> = admitted(&resolved, Some(&enabled))
        .iter()
        .map(|entry| entry.metadata.id.as_str())
        .collect();
    assert_eq!(admitted, ["governance"]);
    let both: EnabledExtensions = [id("slack"), id("notifications")].into_iter().collect();
    assert_eq!(admitted_len(&resolved, &both), 3);
}

fn admitted_len(resolved: &[Resolved<&'static str>], enabled: &EnabledExtensions) -> usize {
    admitted(resolved, Some(enabled)).len()
}

struct Section(ExtensionMetadata);

impl EngineExtension for Section {
    fn metadata(&self) -> ExtensionMetadata {
        self.0.clone()
    }

    fn prompt_sections(&self, step: &PromptContext<'_>) -> Vec<PromptSection> {
        vec![PromptSection {
            heading: self.0.id.to_string(),
            text: step.agent.to_owned(),
        }]
    }
}

#[test]
fn contributions_carry_their_extension_and_follow_enablement() {
    let brain: Arc<dyn EngineExtension> = Arc::new(Section(meta("brain", 10)));
    let governance: Arc<dyn EngineExtension> =
        Arc::new(Section(meta("governance", 99).mark_required()));
    let set = ExtensionSet::resolve([brain, governance]).unwrap();
    let session = SessionId::mint();
    let step = PromptContext {
        session_id: &session,
        agent: "build",
        model_id: "model",
        directory: std::path::Path::new("."),
        subagent: false,
    };
    let withheld = set.prompt_sections(None, &step);
    assert_eq!(withheld.len(), 1);
    assert_eq!(withheld[0].extension, id("governance"));
    let enabled: EnabledExtensions = std::iter::once(id("brain")).collect();
    let sections = set.prompt_sections(Some(&enabled), &step);
    let owners: Vec<_> = sections
        .iter()
        .map(|section| section.extension.as_str())
        .collect();
    assert_eq!(owners, ["governance", "brain"]);
    assert_eq!(sections[1].item.text, "build");
    assert!(set.tools(Some(&enabled)).is_empty());
}

#[test]
fn an_extension_header_never_overrides_one_the_call_carries() {
    let mut endpoint = Endpoint::new(url::Url::parse("https://instance.test/v1").unwrap());
    endpoint
        .headers
        .push(("Authorization".to_owned(), "Bearer governed".to_owned()));
    add_headers(
        &mut endpoint,
        &extension_id!("brain"),
        vec![
            ("authorization".to_owned(), "Bearer forged".to_owned()),
            ("x-memory".to_owned(), "off".to_owned()),
        ],
    );
    assert_eq!(
        endpoint.headers,
        [
            ("Authorization".to_owned(), "Bearer governed".to_owned()),
            ("x-memory".to_owned(), "off".to_owned()),
        ]
    );
}
