//! Builds one slot from entries at a chosen standing the way the kernel does:
//! each entry through its own member sink and `take_entry`, grouped and
//! ordered by `build_slot`, inserted through the checked slot insert.

use std::sync::Arc;

use systemprompt_extension::contribute::{build_slot, ContributionSink};
use systemprompt_extension::{Contributions, Resolved, Slot, SlotError, StandingTag};
use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::Dependencies;

pub fn built_slot<S: Slot>(
    entries: Vec<(&str, StandingTag, S::Builder)>,
    r: &Resolved<'_>,
) -> Result<Arc<S::Built>, SlotError> {
    let contributed = !entries.is_empty();
    let mut inputs = Vec::new();
    for (owner, standing, builder) in entries {
        let owner = ExtensionId::new(owner);
        let mut sink = ContributionSink::default();
        let mut c = Contributions::__new(owner.clone(), &mut sink);
        *c.slot::<S>() = builder;
        inputs.extend(
            sink.slots()
                .iter()
                .filter_map(|record| record.take_entry(owner.clone(), standing)),
        );
    }
    let Some(built) = build_slot(inputs, r)? else {
        assert!(
            !contributed,
            "a core-first slot with no core-standing entry is not built; open it with the core owner's entry first"
        );
        return S::build(Vec::new(), r).map(Arc::new);
    };
    let mut state = Dependencies::default();
    built
        .insert_into(&mut state)
        .expect("an empty state holds no slot value");
    Ok(Arc::clone(
        state
            .get::<Arc<S::Built>>()
            .expect("the built slot value is inserted"),
    ))
}
