//! The `register_extension!` macro submitting extensions to the `inventory`
//! registry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[macro_export]
macro_rules! register_extension {
    ($ext_type:ty) => {
        ::inventory::submit! {
            $crate::ExtensionRegistration {
                factory: || ::std::sync::Arc::new(<$ext_type>::default()) as ::std::sync::Arc<dyn $crate::Extension>,
            }
        }
    };
    ($ext_expr:expr) => {
        ::inventory::submit! {
            $crate::ExtensionRegistration {
                factory: || ::std::sync::Arc::new($ext_expr) as ::std::sync::Arc<dyn $crate::Extension>,
            }
        }
    };
}
