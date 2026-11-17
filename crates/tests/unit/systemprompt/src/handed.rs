//! A kernel handle reaches only a route set at a base that confers it: a
//! provide fn, a factory, a slot build, the command host's resolution and a
//! route set at an unconferred base all see it missing, while the same set
//! resolved with the conferral sees it (T4 broadcaster impostor, T20).

use std::sync::Arc;

use systemprompt_extension::contribute::build_slot;
use systemprompt_extension::handles::AnalyticsBroadcaster;
use systemprompt_extension::health::ReadinessCheck;
use systemprompt_extension::router::RouteSet;
use systemprompt_extension::{
    FactoryError, FromState, Handed, HandleId, MissingDependency, ProvideError, Resolved,
    RouteGroup, Slot, SlotEntry, SlotError, SlotOrder,
};
use systemprompt_identifiers::{ConnectionId, UserId};
use systemprompt_runtime::Kernel;
use systemprompt_traits::events::{AnalyticsSubscription, SubscribedStream};

use crate::probe::{Counter, EXT_BASE, PLAIN, Probe, build_with, kernel, ok};

struct NoSubscriptions;

impl AnalyticsSubscription for NoSubscriptions {
    fn subscribe<'a>(
        &'a self,
        _user_id: &'a UserId,
        _connection_id: &'a ConnectionId,
    ) -> futures::future::BoxFuture<'a, Option<SubscribedStream>> {
        Box::pin(std::future::ready(None))
    }
}

fn handing_kernel() -> Kernel {
    kernel().with_handle::<AnalyticsBroadcaster>(Arc::new(NoSubscriptions))
}

fn refused(r: &Resolved<'_>) -> bool {
    Handed::<AnalyticsBroadcaster>::from_state(r).is_err()
}

#[derive(Default)]
struct Refusals {
    provide: Counter,
    factory: Counter,
    slot: Counter,
}

#[derive(Default)]
struct RefusalBuilder {
    refusals: Option<Arc<Refusals>>,
}

struct HandleProbeSlot;

impl Slot for HandleProbeSlot {
    type Builder = RefusalBuilder;
    type Built = ();
    const ORDER: SlotOrder = SlotOrder::Composition;

    fn build(entries: Vec<SlotEntry<RefusalBuilder>>, r: &Resolved<'_>) -> Result<(), SlotError> {
        for entry in entries {
            if let Some(refusals) = entry.builder.refusals
                && refused(r)
            {
                refusals.slot.hit();
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
struct HandleProbeValue;

#[test]
fn handle_is_unreachable_outside_a_conferred_route_set() {
    let refusals = Arc::new(Refusals::default());
    let shared = Arc::clone(&refusals);
    let admitted = build_with(Probe::new("probe-handle", move |c| {
        let provide = Arc::clone(&shared);
        c.provide::<HandleProbeValue, _>(
            move |r: &Resolved<'_>| -> Result<HandleProbeValue, ProvideError> {
                if refused(r) {
                    provide.provide.hit();
                }
                Ok(HandleProbeValue)
            },
        );
        let factory = Arc::clone(&shared);
        c.readiness(
            move |r: &Resolved<'_>| -> Result<Arc<dyn ReadinessCheck>, FactoryError> {
                if refused(r) {
                    factory.factory.hit();
                }
                Err(FactoryError::Missing(MissingDependency::of::<()>()))
            },
        );
        c.slot::<HandleProbeSlot>().refusals = Some(Arc::clone(&shared));
        c.routes(
            RouteSet::at::<Handed<AnalyticsBroadcaster>>(EXT_BASE, RouteGroup::Extensions)
                .route(http::Method::GET, "/stream", ok, PLAIN)
                .build(),
        );
    }))
    .expect("the handle request is opaque to admission");

    let kernel = handing_kernel();
    let resolved = kernel.resolved();
    assert!(
        refused(&resolved),
        "the command host resolves through Kernel::resolved"
    );

    let member = admitted
        .built()
        .members()
        .find(|m| m.owner.as_str() == "probe-handle")
        .expect("the probe is admitted");
    for entry in member.sink.provides() {
        assert!(entry.compute(&resolved).is_ok());
    }
    for factory in member.sink.readiness() {
        assert!(factory.build(&resolved).is_err());
    }
    let inputs: Vec<_> = member
        .sink
        .slots()
        .iter()
        .filter_map(|record| record.take_entry(member.owner.clone(), member.standing))
        .collect();
    assert!(build_slot(inputs, &resolved).is_ok());
    assert_eq!(
        refusals.provide.get(),
        1,
        "a provide fn never sees a handle"
    );
    assert_eq!(refusals.factory.get(), 1, "a factory never sees a handle");
    assert_eq!(refusals.slot.get(), 1, "a slot build never sees a handle");

    let set = admitted
        .routes()
        .iter()
        .find(|set| set.owner.as_str() == "probe-handle")
        .expect("the probe's route set is admitted");
    assert!(set.handles.is_empty(), "an install base confers no handle");
    assert!(
        set.set
            .materialize(&kernel.route_scoped(&set.handles))
            .is_err(),
        "a route set at an unconferred base cannot resolve the handle"
    );
    assert!(
        set.set
            .materialize(&kernel.route_scoped(&[HandleId::AnalyticsBroadcaster]))
            .is_ok(),
        "negative control: the same set resolves with the conferral"
    );
}
