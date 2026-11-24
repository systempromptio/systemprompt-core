//! The late cells a core member declares.
//!
//! A value some members read while the kernel resolves but that another
//! member can only build after them (the document store is built from slot
//! outputs of the resolve the files member provides into) is read through a
//! late cell. A core member declares the cell type with
//! `Contributions::late_cell`; the kernel creates it before any member
//! provides, inserts its read half into the kernel state and keeps the bind
//! half itself. Once the composition has resolved, the kernel binds the cell
//! to the `Arc<LateCell::Value>` the state then holds; with no such value the
//! cell stays unbound and every read through it fails closed. A host that
//! seeded the cell's read half itself keeps it and binds it with the half it
//! holds; the kernel then creates and binds nothing. No member can
//! bind a cell: the bind half never enters the state, and the read half is a
//! type the kernel already holds, so no member can provide it. An install's
//! late cell is refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::type_name;
use std::sync::Arc;

use systemprompt_models::AlreadyBound;
use systemprompt_provider_contracts::Dependencies;

/// A cell whose read half the kernel seeds and whose bind half the kernel
/// alone holds; see the module docs.
pub trait LateCell: Clone + Send + Sync + 'static {
    type Value: ?Sized + Send + Sync + 'static;
    type Binder: Send + 'static;

    fn unbound() -> (Self, Self::Binder);

    fn bind(binder: Self::Binder, value: Arc<Self::Value>) -> Result<(), AlreadyBound>;
}

pub type PendingBind = Box<dyn FnOnce(&Dependencies) -> Result<(), AlreadyBound> + Send>;

/// One declared late cell: its type and the monomorphised seed.
#[derive(Debug, Clone, Copy)]
pub struct LateCellEntry {
    type_name: &'static str,
    seed: fn(&mut Dependencies) -> PendingBind,
}

impl LateCellEntry {
    pub(crate) fn of<C: LateCell>() -> Self {
        Self {
            type_name: type_name::<C>(),
            seed: seed::<C>,
        }
    }

    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        self.type_name
    }

    pub fn seed_into(&self, state: &mut Dependencies) -> PendingBind {
        (self.seed)(state)
    }
}

fn seed<C: LateCell>(state: &mut Dependencies) -> PendingBind {
    if state.find::<C>().is_some() {
        return Box::new(|_: &Dependencies| Ok(()));
    }
    let (read, binder) = C::unbound();
    state.insert(read);
    Box::new(move |state: &Dependencies| {
        state
            .find::<Arc<C::Value>>()
            .map_or(Ok(()), |value| C::bind(binder, Arc::clone(value)))
    })
}
