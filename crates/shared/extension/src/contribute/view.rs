//! The read-only kernel view a member declares against.
//!
//! `contribute` runs once, before the profile is loaded, because the CLI
//! builds its command tree from the declared commands before it reads
//! `--profile`. The view therefore carries no profile-derived data, no
//! secrets and no configuration: route shape cannot depend on the profile.
//! Assets that need the profile's paths are declared as a deferred closure
//! (`Contributions::asset_with`) and evaluated at admission.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::marker::PhantomData;

#[derive(Debug, Clone, Copy)]
pub struct KernelView<'a> {
    _scope: PhantomData<&'a ()>,
}

impl KernelView<'_> {
    #[doc(hidden)]
    #[must_use]
    pub const fn __new() -> Self {
        Self {
            _scope: PhantomData,
        }
    }
}
