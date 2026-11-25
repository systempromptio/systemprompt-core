//! An install member's gateway guard, authz hook entry and subject provider
//! are built before the kernel inserts the subject provider set. Each holds
//! the `SubjectSourceCell` instead: unbound (fail closed) while the factory
//! runs, bound to the set the kernel built once the composition resolves, so
//! the guard reads every dimension, the install's own included.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use systemprompt_extension::{
    GatewayDenyReason, GatewayGuardRequest, GatewayRequestGuard, Resolved,
};
use systemprompt_governance::authz::{
    AllowAllHook, RuleType, SharedAuthzHook, SubjectAttributeError, SubjectAttributeProvider,
    SubjectAttributeSource, SubjectDimension, SubjectProviderSet, SubjectSourceCell,
    SubjectSourceError,
};
use systemprompt_governance::contract::AuthzHookSlot;
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{
    ComposedParts, Probe, authz_probe_admitted, offline_resolved_kernel,
};
use systemprompt_traits::DatabaseHandle;

const DIMENSION: &str = "fx_cost_centre";

#[derive(Debug)]
struct CostCentre;

#[async_trait]
impl SubjectAttributeProvider for CostCentre {
    fn dimension(&self) -> SubjectDimension {
        SubjectDimension {
            rule_type: RuleType::extension_static(DIMENSION),
            label: "Cost centre",
            precedence: 300,
        }
    }

    async fn values_for(&self, _user: &UserId) -> Result<Vec<String>, SubjectAttributeError> {
        Ok(vec!["cc-7".to_owned()])
    }
}

#[derive(Debug)]
struct ReadsSubjects;

#[async_trait]
impl GatewayRequestGuard for ReadsSubjects {
    async fn check(
        &self,
        _database: &dyn DatabaseHandle,
        _request: &GatewayGuardRequest<'_>,
    ) -> Result<(), GatewayDenyReason> {
        Ok(())
    }
}

#[derive(Default)]
struct Seen {
    guard: Option<SubjectSourceCell>,
    hook: Option<SubjectSourceCell>,
    provider: Option<SubjectSourceCell>,
    bound_at_build: Vec<bool>,
    set_at_guard_build: Option<bool>,
}

fn probe(seen: &Arc<Mutex<Seen>>) -> Probe {
    let seen = Arc::clone(seen);
    Probe::new("fx-subject-reader", move |c| {
        let on_guard = Arc::clone(&seen);
        let on_hook = Arc::clone(&seen);
        let on_provider = Arc::clone(&seen);
        c.subject_attributes(move |r: &Resolved<'_>| {
            let cell = r.get::<SubjectSourceCell>()?;
            let mut seen = on_provider.lock().expect("seen");
            seen.bound_at_build.push(cell.get().is_ok());
            seen.provider = Some(cell);
            let provider: Arc<dyn SubjectAttributeProvider> = Arc::new(CostCentre);
            Ok(provider)
        })
        .gateway_guard(move |r: &Resolved<'_>| {
            let cell = r.get::<SubjectSourceCell>()?;
            let mut seen = on_guard.lock().expect("seen");
            seen.set_at_guard_build = Some(r.get::<Arc<SubjectProviderSet>>().is_ok());
            seen.bound_at_build.push(cell.get().is_ok());
            seen.guard = Some(cell);
            let guard: Arc<dyn GatewayRequestGuard> = Arc::new(ReadsSubjects);
            Ok(guard)
        });
        c.slot::<AuthzHookSlot>().hook(move |r: &Resolved<'_>| {
            let cell = r.get::<SubjectSourceCell>()?;
            let mut seen = on_hook.lock().expect("seen");
            seen.bound_at_build.push(cell.get().is_ok());
            seen.hook = Some(cell);
            let hook: SharedAuthzHook = Arc::new(AllowAllHook::null());
            Ok(hook)
        });
    })
}

fn dimension() -> SubjectDimension {
    CostCentre.dimension()
}

#[tokio::test]
async fn install_factories_read_the_subject_set_through_the_cell_once_resolved() {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let admitted = authz_probe_admitted(ComposedParts {
        probes: vec![probe(&seen)],
        ..ComposedParts::default()
    });
    let _kernel = offline_resolved_kernel(&admitted, Default::default());
    let seen = seen.lock().expect("seen");

    assert_eq!(seen.bound_at_build, vec![false, false, false]);
    assert_eq!(seen.set_at_guard_build, Some(false));

    let user = UserId::new("fx-user");
    for cell in [&seen.guard, &seen.hook, &seen.provider] {
        let cell = cell.as_ref().expect("every factory ran");
        let set = cell
            .get()
            .expect("the kernel binds the cell once it resolves");
        let names: Vec<&str> = set
            .dimensions()
            .iter()
            .map(|d| d.rule_type.as_str())
            .collect();
        assert!(names.contains(&DIMENSION), "{names:?}");
        assert!(names.contains(&"traffic"), "{names:?}");
        assert_eq!(
            set.values_for(&user, &dimension()).await.expect("resolves"),
            vec!["cc-7".to_owned()]
        );
    }
}

#[tokio::test]
async fn an_unbound_cell_withholds_the_set() {
    use systemprompt_extension::LateCell;
    let (cell, _binder) = SubjectSourceCell::unbound();
    assert!(matches!(cell.get(), Err(SubjectSourceError::NotReady)));
}
