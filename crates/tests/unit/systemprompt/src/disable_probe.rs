//! A disabled member contributes nothing: admission drops its routes,
//! provide fns, factories and slot entries before any of them runs, and an
//! enabled twin's run as the negative control (T11).

use std::sync::Arc;

use systemprompt::reference;
use systemprompt_extension::cli::CommandDecl;
use systemprompt_extension::contribute::build_slot;
use systemprompt_extension::health::ReadinessCheck;
use systemprompt_extension::router::{BasePath, WebBase};
use systemprompt_extension::{
    Admitted, FactoryError, MemberResolved, MissingDependency, Resolved, Slot, SlotEntry,
    SlotError, SlotOrder,
};

use crate::cli_probe::ProbeGroup;
use crate::probe::{Counter, EXT_BASE, Probe, kernel, plain_set, profile};

#[derive(Default)]
struct Hits {
    provide: Counter,
    factory: Counter,
    slot: Counter,
}

impl Hits {
    fn total(&self) -> usize {
        self.provide.get() + self.factory.get() + self.slot.get()
    }
}

#[derive(Default)]
struct CountingBuilder {
    hits: Option<Arc<Hits>>,
}

struct CountingSlot;

impl Slot for CountingSlot {
    type Builder = CountingBuilder;
    type Built = usize;
    const ORDER: SlotOrder = SlotOrder::Composition;

    fn build(
        entries: Vec<SlotEntry<CountingBuilder>>,
        _r: &Resolved<'_>,
    ) -> Result<usize, SlotError> {
        for entry in &entries {
            if let Some(hits) = &entry.builder.hits {
                hits.slot.hit();
            }
        }
        Ok(entries.len())
    }
}

#[derive(Clone)]
struct OnValue;

#[derive(Clone)]
struct OffValue;

fn member<V: Clone + Send + Sync + 'static>(
    id: &'static str,
    value: V,
    hits: &Arc<Hits>,
    base: BasePath,
    with_command: bool,
) -> Probe {
    let hits = Arc::clone(hits);
    Probe::new(id, move |c| {
        let provide = Arc::clone(&hits);
        let value = value.clone();
        c.provide::<V, _>(move |_: &Resolved<'_>| {
            provide.provide.hit();
            Ok(value.clone())
        });
        let factory = Arc::clone(&hits);
        c.readiness(
            move |_: &Resolved<'_>| -> Result<Arc<dyn ReadinessCheck>, FactoryError> {
                factory.factory.hit();
                Err(FactoryError::Missing(MissingDependency::of::<()>()))
            },
        );
        c.slot::<CountingSlot>().hits = Some(Arc::clone(&hits));
        c.routes(plain_set(base, "/probe"));
        if with_command {
            c.command(CommandDecl::group::<ProbeGroup>());
        }
    })
}

fn run_admitted_probes(admitted: &Admitted) {
    let kernel = kernel();
    let resolved = kernel.resolved();
    let mut inputs = Vec::new();
    for member in admitted.built().members() {
        if !member.owner.as_str().starts_with("probe-") {
            continue;
        }
        for entry in member.sink.provides() {
            assert!(entry.compute(&resolved).is_ok());
        }
        let scope = member.scope();
        for factory in member.sink.readiness() {
            assert!(
                factory
                    .build(&MemberResolved::within(&resolved, &scope))
                    .is_err()
            );
        }
        for record in member.sink.slots() {
            inputs.extend(record.take_entry(member.owner.clone(), member.standing));
        }
    }
    assert!(build_slot(inputs, &resolved).is_ok());
}

#[test]
fn disabled_member_contributes_nothing_and_runs_nothing() {
    let on = Arc::new(Hits::default());
    let off = Arc::new(Hits::default());
    let declared = reference()
        .with(member("probe-on", OnValue, &on, EXT_BASE, false))
        .with(member(
            "probe-off",
            OffValue,
            &off,
            BasePath::Web(WebBase::Root),
            true,
        ))
        .declare()
        .expect("both probes declare");

    assert!(
        declared
            .commands()
            .commands()
            .iter()
            .any(|cmd| cmd.owner.as_str() == "probe-off"),
        "commands are declared before the profile; the CLI refuses a disabled owner at dispatch"
    );

    let admitted = declared
        .admit(&profile(&["probe-off"]).extensions)
        .expect("admission succeeds");
    assert_eq!(
        on.total() + off.total(),
        0,
        "declaration and admission only store"
    );

    assert!(
        !admitted
            .built()
            .members()
            .any(|m| m.owner.as_str() == "probe-off")
    );
    assert!(
        !admitted
            .routes()
            .iter()
            .any(|set| set.owner.as_str() == "probe-off")
    );
    assert!(
        admitted
            .built()
            .members()
            .any(|m| m.owner.as_str() == "probe-on")
    );
    assert!(
        admitted
            .routes()
            .iter()
            .any(|set| set.owner.as_str() == "probe-on")
    );

    run_admitted_probes(&admitted);

    assert_eq!(
        off.provide.get(),
        0,
        "a disabled member's provide fn never runs"
    );
    assert_eq!(
        off.factory.get(),
        0,
        "a disabled member's factory never runs"
    );
    assert_eq!(
        off.slot.get(),
        0,
        "a disabled member's slot entry is never built"
    );
    assert_eq!(on.provide.get(), 1, "the enabled member's provide fn runs");
    assert_eq!(on.factory.get(), 1, "the enabled member's factory runs");
    assert_eq!(on.slot.get(), 1, "the enabled member's slot entry is built");
}

#[test]
fn enabled_profile_admits_both_probes() {
    let on = Arc::new(Hits::default());
    let off = Arc::new(Hits::default());
    let admitted = reference()
        .with(member("probe-on", OnValue, &on, EXT_BASE, false))
        .with(member(
            "probe-off",
            OffValue,
            &off,
            BasePath::Web(WebBase::Root),
            true,
        ))
        .build(&profile(&[]))
        .expect("admission succeeds");
    run_admitted_probes(&admitted);
    assert_eq!(off.total(), 3);
    assert_eq!(on.total(), 3);
}
