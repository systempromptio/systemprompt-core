//! Probe extensions and fixtures shared by the composition acceptance tests.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use systemprompt::{Composition, CompositionError, reference};
use systemprompt_extension::reserved::{CoreId, PRIVILEGED_SHAPES, PrivilegedShape};
use systemprompt_extension::router::{
    Allowance, ApiBase, BasePath, BaseRef, BodyLimit, RouteLimits, RouteSet, RouteSpec,
};
use systemprompt_extension::{
    Admitted, AuthzPolicy, ContextKind, ContributionError, Contributions, Extension,
    ExtensionMetadata, FromState, MissingDependency, RateBudget, RateLimit, Resolved, RouteGroup,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_manifest::Profile;
use systemprompt_provider_contracts::Dependencies;
use systemprompt_runtime::Kernel;
use systemprompt_traits::{BackgroundTasks, DatabaseHandle};

type ContributeFn = Arc<dyn Fn(&mut Contributions<'_>) + Send + Sync>;

pub(crate) struct Probe {
    id: &'static str,
    contribute: ContributeFn,
}

impl Probe {
    pub(crate) fn new(
        id: &'static str,
        f: impl Fn(&mut Contributions<'_>) + Send + Sync + 'static,
    ) -> Self {
        Self {
            id,
            contribute: Arc::new(f),
        }
    }
}

impl Extension for Probe {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: self.id,
            version: "0.0.0",
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        (self.contribute)(c);
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ProbeState;

impl FromState for ProbeState {
    fn from_state(_r: &Resolved<'_>) -> Result<Self, MissingDependency> {
        Ok(Self)
    }
}

pub(crate) async fn ok() -> &'static str {
    "ok"
}

pub(crate) const PLAIN_LIMITS: RouteLimits = RouteLimits {
    rate: RateLimit::new("probe", RateBudget::Contexts),
    body: BodyLimit::Default,
};

pub(crate) const PLAIN: RouteSpec =
    RouteSpec::new(AuthzPolicy::user(), ContextKind::User, PLAIN_LIMITS);

pub(crate) const EXT_BASE: BasePath = BasePath::Api(ApiBase::Extension("/api/probe"));

pub(crate) fn shape(
    owner: CoreId,
    base: BaseRef,
    allowance: Allowance,
) -> &'static PrivilegedShape {
    PRIVILEGED_SHAPES
        .iter()
        .find(|entry| entry.owner == owner && entry.base == base && entry.allowance == allowance)
        .expect("the privileged shape is listed")
}

pub(crate) fn plain_set(base: BasePath, path: &'static str) -> RouteSet {
    RouteSet::at::<ProbeState>(base, RouteGroup::Extensions)
        .route(http::Method::GET, path, ok, PLAIN)
        .build()
}

pub(crate) fn profile(disabled: &[&str]) -> Profile {
    let mut profile = (*systemprompt_test_fixtures::test_boot_profile()).clone();
    profile.extensions.disabled = disabled.iter().map(|id| ExtensionId::new(*id)).collect();
    profile
}

pub(crate) fn build(composition: Composition) -> Result<Admitted, CompositionError> {
    composition.build(&profile(&[]))
}

pub(crate) fn build_with(probe: Probe) -> Result<Admitted, CompositionError> {
    build(reference().with(probe))
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

    fn into_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        self
    }
}

pub(crate) fn kernel() -> Kernel {
    kernel_with(Dependencies::default())
}

pub(crate) fn kernel_with(dependencies: Dependencies) -> Kernel {
    Kernel::new(Arc::new(StubDb), BackgroundTasks::new(), dependencies)
}

#[derive(Debug, Default)]
pub(crate) struct Counter(AtomicUsize);

impl Counter {
    pub(crate) fn hit(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }

    pub(crate) fn get(&self) -> usize {
        self.0.load(Ordering::SeqCst)
    }
}
