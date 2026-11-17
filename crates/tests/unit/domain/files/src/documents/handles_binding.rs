use std::sync::Arc;

use systemprompt_files::documents::handles::bound;
use systemprompt_files::documents::{
    DocumentsHandlesBinder, DocumentsHandlesSlot, DocumentsUnbound,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::{Dependencies, DependencyProvider};

fn host_state() -> (Dependencies, DocumentsHandlesSlot, DocumentsHandlesBinder) {
    let (slot, binder) = DocumentsHandlesSlot::unbound();
    (Dependencies::new().with(slot.clone()), slot, binder)
}

#[test]
fn a_member_cannot_provide_the_slot_the_host_seeds() {
    let (mut state, _, _binder) = host_state();
    let (forged, _forged_binder) = DocumentsHandlesSlot::unbound();

    let refused = state
        .provide(&ExtensionId::new("intruder"), Arc::new(forged))
        .expect_err("the kernel already holds the slot type");

    assert_eq!(refused.first, DependencyProvider::Host);
    assert_eq!(refused.second.as_str(), "intruder");
}

#[test]
fn the_bind_half_is_never_in_the_state_a_member_reads() {
    let (state, _, _binder) = host_state();

    assert!(
        state.get::<DocumentsHandlesBinder>().is_err(),
        "the binder is a MissingDependency to every reader of the state"
    );
    assert!(
        state.get::<DocumentsHandlesSlot>().is_ok(),
        "control: the read half is in the state"
    );
}

#[test]
fn a_seeded_slot_reads_as_unbound_until_the_host_binds() {
    let (state, _, _binder) = host_state();
    let slot = state.get::<DocumentsHandlesSlot>().expect("seeded");

    assert!(matches!(bound(slot), Err(DocumentsUnbound)));
}
