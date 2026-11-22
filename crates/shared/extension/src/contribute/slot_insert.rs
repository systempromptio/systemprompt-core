//! Inserting a built slot value into the state, sealed when its slot is.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::{Dependencies, DuplicateDependency};

use super::slot::{ErasedBuilt, Slot};

pub(super) fn insert_erased<S: Slot>(
    state: &mut Dependencies,
    owner: &ExtensionId,
    value: ErasedBuilt,
) -> Result<(), DuplicateDependency> {
    value.downcast::<S::Built>().map_or(Ok(()), |built| {
        if S::SEALED {
            state.provide_sealed(owner, Arc::new(built))
        } else {
            state.provide(owner, Arc::new(built))
        }
    })
}
