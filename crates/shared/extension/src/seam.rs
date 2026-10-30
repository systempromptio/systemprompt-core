//! The shared shape of every inventory-collected extension seam: a
//! registration struct carrying a `priority` and a factory over the
//! [`ExtensionContext`](crate::ExtensionContext), a `discover_*` function
//! returning registrations sorted by ascending priority, and the
//! `__register_seam!` macro the public `register_*!` macros forward to.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

macro_rules! seam_registration {
    ($registration:ident, $trait:ident, $discover:ident) => {
        /// Compile-time registration of an extension seam, collected via
        /// `inventory`.
        #[derive(Debug, Clone, Copy)]
        pub struct $registration {
            pub priority: i32,
            pub factory: fn(&dyn $crate::context::ExtensionContext) -> ::std::sync::Arc<dyn $trait>,
        }

        $crate::__inventory::collect!($registration);

        #[must_use]
        pub fn $discover() -> Vec<&'static $registration> {
            let mut registrations: Vec<&'static $registration> =
                $crate::__inventory::iter::<$registration>
                    .into_iter()
                    .collect();
            registrations.sort_by_key(|registration| registration.priority);
            registrations
        }
    };
}

pub(crate) use seam_registration;

#[doc(hidden)]
#[macro_export]
macro_rules! __register_seam {
    ($module:ident, $registration:ident, $trait:ident, factory = $factory:expr, priority = $priority:expr) => {
        $crate::__inventory::submit! {
            $crate::$module::$registration {
                priority: $priority,
                factory: $factory,
            }
        }
    };
    ($module:ident, $registration:ident, $trait:ident, $ty:ty, priority = $priority:expr) => {
        $crate::__inventory::submit! {
            $crate::$module::$registration {
                priority: $priority,
                factory: |_| ::std::sync::Arc::new(<$ty>::default())
                    as ::std::sync::Arc<dyn $crate::$module::$trait>,
            }
        }
    };
}
