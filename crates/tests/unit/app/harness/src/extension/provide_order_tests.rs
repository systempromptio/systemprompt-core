//! `provide` runs each extension after the extensions it depends on.

use std::sync::Arc;

use systemprompt_harness::extension::{ExtensionMetadata, SpContext, SpExtension, SpExtensions};

use crate::extension::support::{context, id};

struct Provided;

struct SawDependency(bool);

struct Dependency;

impl SpExtension for Dependency {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata::new(id("zeta"), "zeta").with_priority(10)
    }

    fn provide(&self, ctx: &SpContext) {
        ctx.handles.insert(Arc::new(Provided));
    }
}

struct Dependent;

impl SpExtension for Dependent {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata::new(id("alpha"), "alpha")
            .with_priority(900)
            .depends_on(id("zeta"))
    }

    fn provide(&self, ctx: &SpContext) {
        let seen = ctx.handles.contains::<Provided>();
        ctx.handles.insert(Arc::new(SawDependency(seen)));
    }
}

#[test]
fn a_dependency_provides_before_a_higher_priority_dependent() {
    let linked: [Arc<dyn SpExtension>; 2] = [Arc::new(Dependent), Arc::new(Dependency)];
    let extensions = SpExtensions::from_extensions(linked).unwrap();
    let resolved: Vec<_> = extensions
        .entries()
        .iter()
        .map(|entry| entry.metadata.id.as_str())
        .collect();
    assert_eq!(resolved, ["alpha", "zeta"]);
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    extensions.provide(&ctx);
    let saw = ctx.handles.get::<SawDependency>().unwrap();
    assert!(saw.0, "the dependent provided before its dependency");
}
