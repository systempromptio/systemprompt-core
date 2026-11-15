//! The brain and notifications capabilities as the reference composition
//! admits them: each member opens its own slot, so its built seams exist
//! exactly when it is admitted; contributed seams build in composition
//! order; a disabled member's slot entry is never built while an enabled
//! twin's is (the negative control); a failing seam factory refuses the
//! build. `notifications-http` holds its two route sets and one gateway
//! route whose limits carry the bearer window (T18, facade side).

use std::any::TypeId;
use std::sync::Arc;

use systemprompt::brain::{BrainSeams, BrainSlot};
use systemprompt::models::brain::BrainEnvelope;
use systemprompt::notifications::{NotificationSeams, NotificationsSlot};
use systemprompt::reference;
use systemprompt_extension::contribute::build_slot;
use systemprompt_extension::reserved::CoreId;
use systemprompt_extension::router::{
    Allowance, ApiBase, BasePath, BaseRef, GatewayPath, RouteSpec,
};
use systemprompt_extension::{
    Admitted, EventFilter, FactoryError, FilterVerdict, NotificationRouter, Resolved, RouteGroup,
    Slot, SlotError,
};
use systemprompt_provider_contracts::Dependencies;

use crate::probe::{Counter, Probe, build, kernel, profile, shape};

struct NamedFilter(&'static str);

impl EventFilter for NamedFilter {
    fn name(&self) -> &'static str {
        self.0
    }

    fn decide(&self, _: &BrainEnvelope) -> FilterVerdict {
        FilterVerdict::Ignore
    }
}

fn filter_probe(id: &'static str, name: &'static str, hits: &Arc<Counter>) -> Probe {
    let hits = Arc::clone(hits);
    Probe::new(id, move |c| {
        let hits = Arc::clone(&hits);
        c.slot::<BrainSlot>().filter(
            move |_: &Resolved<'_>| -> Result<Arc<dyn EventFilter>, FactoryError> {
                hits.hit();
                Ok(Arc::new(NamedFilter(name)))
            },
        );
    })
}

fn failing_router_probe(id: &'static str, hits: &Arc<Counter>) -> Probe {
    let hits = Arc::clone(hits);
    Probe::new(id, move |c| {
        let hits = Arc::clone(&hits);
        c.slot::<NotificationsSlot>().router(
            move |_: &Resolved<'_>| -> Result<Arc<dyn NotificationRouter>, FactoryError> {
                hits.hit();
                Err(FactoryError::Failed("probe router refuses".into()))
            },
        );
    })
}

fn slot_owners<S: Slot>(admitted: &Admitted) -> Vec<String> {
    admitted
        .built()
        .members()
        .filter(|member| {
            member
                .sink
                .slots()
                .iter()
                .any(|record| record.slot() == TypeId::of::<S>())
        })
        .map(|member| member.owner.as_str().to_owned())
        .collect()
}

fn built<S: Slot>(admitted: &Admitted) -> Result<Option<Arc<S::Built>>, SlotError>
where
    S::Built: Clone,
{
    let kernel = kernel();
    let resolved = kernel.resolved();
    let mut inputs = Vec::new();
    for member in admitted.built().members() {
        for record in member.sink.slots() {
            if record.slot() == TypeId::of::<S>() {
                inputs.extend(record.take_entry(member.owner.clone(), member.standing));
            }
        }
    }
    let mut state = Dependencies::default();
    if let Some(value) = build_slot(inputs, &resolved)? {
        value.insert_into(&mut state);
    }
    Ok(state.find::<Arc<S::Built>>().cloned())
}

fn filter_names(seams: &BrainSeams) -> Vec<&'static str> {
    seams.filters.iter().map(|f| f.name()).collect()
}

#[test]
fn brain_and_notifications_open_their_own_slots() {
    let admitted = build(reference()).expect("the reference composition admits");
    assert_eq!(slot_owners::<BrainSlot>(&admitted), ["brain"]);
    assert_eq!(
        slot_owners::<NotificationsSlot>(&admitted),
        ["notifications"]
    );

    let brain = built::<BrainSlot>(&admitted)
        .expect("the brain slot builds")
        .expect("the brain member's slot is built");
    assert!(filter_names(&brain).is_empty(), "core contributes no seam");
    let notifications = built::<NotificationsSlot>(&admitted)
        .expect("the notifications slot builds")
        .expect("the notifications member's slot is built");
    assert!(notifications.routers.is_empty(), "core contributes no seam");
}

#[test]
fn disabled_capability_members_leave_no_slot() {
    let admitted = reference()
        .declare()
        .expect("the reference composition declares")
        .admit(&profile(&["brain", "notifications", "notifications-http"]))
        .expect("disabling the brain and notifications members admits");
    assert!(slot_owners::<BrainSlot>(&admitted).is_empty());
    assert!(slot_owners::<NotificationsSlot>(&admitted).is_empty());
    assert!(
        built::<BrainSlot>(&admitted)
            .expect("an empty slot builds nothing")
            .is_none()
    );
    assert!(
        built::<NotificationsSlot>(&admitted)
            .expect("an empty slot builds nothing")
            .is_none()
    );
}

#[test]
fn contributed_brain_seams_build_in_composition_order() {
    let first = Arc::new(Counter::default());
    let second = Arc::new(Counter::default());
    let admitted = build(
        reference()
            .with(filter_probe("probe-first", "first", &first))
            .with(filter_probe("probe-second", "second", &second)),
    )
    .expect("both probes admit");
    assert_eq!(first.get() + second.get(), 0, "admission only stores");
    let seams = built::<BrainSlot>(&admitted)
        .expect("the brain slot builds")
        .expect("the brain slot is built");
    assert_eq!(filter_names(&seams), ["first", "second"]);
}

#[test]
fn disabled_member_brain_seam_is_never_built() {
    let on = Arc::new(Counter::default());
    let off = Arc::new(Counter::default());
    let admitted = reference()
        .with(filter_probe("probe-on", "on", &on))
        .with(filter_probe("probe-off", "off", &off))
        .declare()
        .expect("both probes declare")
        .admit(&profile(&["probe-off"]))
        .expect("admission succeeds");
    let seams = built::<BrainSlot>(&admitted)
        .expect("the brain slot builds")
        .expect("the brain slot is built");
    assert_eq!(filter_names(&seams), ["on"]);
    assert_eq!(off.get(), 0, "a disabled member's seam factory never runs");
    assert_eq!(on.get(), 1, "the enabled member's seam factory runs");
}

#[test]
fn disabled_member_notification_seam_is_never_built() {
    let off = Arc::new(Counter::default());
    let admitted = reference()
        .with(failing_router_probe("probe-off", &off))
        .declare()
        .expect("the probe declares")
        .admit(&profile(&["probe-off"]))
        .expect("admission succeeds");
    let seams: Arc<NotificationSeams> = built::<NotificationsSlot>(&admitted)
        .expect("the disabled member's failing factory is dropped before the build")
        .expect("the notifications slot is built");
    assert!(seams.routers.is_empty());
    assert_eq!(off.get(), 0, "a disabled member's seam factory never runs");
}

#[test]
fn failing_notification_seam_refuses_the_build() {
    let hits = Arc::new(Counter::default());
    let admitted = build(reference().with(failing_router_probe("probe-failing", &hits)))
        .expect("the probe admits");
    let error = built::<NotificationsSlot>(&admitted).expect_err("a failing seam refuses");
    assert!(
        matches!(
            &error,
            SlotError::Entry { slot: "notifications", owner, .. } if owner.as_str() == "probe-failing"
        ),
        "{error:?}"
    );
    assert_eq!(hits.get(), 1, "negative control: the enabled factory runs");
}

#[test]
fn notifications_http_holds_its_two_route_sets() {
    let admitted = build(reference()).expect("the reference composition admits");
    let sets: Vec<_> = admitted
        .routes()
        .iter()
        .filter(|set| set.owner.as_str() == "notifications-http")
        .collect();
    assert_eq!(sets.len(), 2, "{sets:?}");
    let at = |base: BasePath| {
        sets.iter()
            .find(|set| set.set.base() == base)
            .unwrap_or_else(|| panic!("notifications-http holds a set at {base:?}"))
    };
    let user = at(BasePath::Api(ApiBase::Notifications));
    assert_eq!(user.set.group(), RouteGroup::ContentAndMisc);
    assert_eq!(user.set.defs().len(), 12);
    let admin = at(BasePath::Api(ApiBase::AdminNotifications));
    assert_eq!(admin.set.group(), RouteGroup::ContentAndMisc);
    assert_eq!(admin.set.defs().len(), 1);
    assert!(user.handles.is_empty() && admin.handles.is_empty());
}

#[test]
fn bridge_notifications_gateway_route_carries_the_bearer_window() {
    let admitted = build(reference()).expect("the reference composition admits");
    let routes: Vec<_> = admitted
        .gateway_routes()
        .iter()
        .filter(|route| route.owner.as_str() == "notifications-http")
        .collect();
    assert_eq!(routes.len(), 1, "one gateway route for the bridge inbox");
    let route = routes[0];
    assert_eq!(route.decl.path, GatewayPath::BridgeNotifications);
    assert!(
        route.decl.limits.rate.bearer_window,
        "the admitted limits carry the bearer window"
    );

    let listed = shape(
        CoreId::NotificationsHttp,
        BaseRef::Gateway(GatewayPath::BridgeNotifications),
        Allowance::GatewayRoute,
    );
    assert_eq!(route.decl.limits, listed.shape.limits);
    assert_eq!(route.decl.router.group(), listed.group);
    let listed_spec = RouteSpec::new(
        listed.shape.policy,
        listed.shape.context,
        listed.shape.limits,
    );
    assert_eq!(route.decl.router.defs().len(), 8);
    assert!(
        route
            .decl
            .router
            .defs()
            .iter()
            .all(|def| def.spec == listed_spec),
        "every bridge def carries the conferred handler-verified shape"
    );
}
