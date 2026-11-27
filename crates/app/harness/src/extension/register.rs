//! Link-time registration of instance-side extensions over `inventory`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use crate::extension::SpExtension;

/// One registered extension type: the factory that builds it with `Default`.
#[derive(Clone, Copy)]
pub struct SpExtensionRegistration {
    pub factory: fn() -> Arc<dyn SpExtension>,
}

impl fmt::Debug for SpExtensionRegistration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SpExtensionRegistration")
            .finish_non_exhaustive()
    }
}

inventory::collect!(SpExtensionRegistration);

#[macro_export]
macro_rules! register_sp_extension {
    ($extension:ty) => {
        $crate::__inventory::submit! {
            $crate::SpExtensionRegistration {
                factory: || {
                    ::std::sync::Arc::new(<$extension as ::core::default::Default>::default())
                        as ::std::sync::Arc<dyn $crate::SpExtension>
                },
            }
        }
    };
}
