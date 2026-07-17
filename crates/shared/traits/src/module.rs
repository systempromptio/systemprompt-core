//! Compile-time module registration macro.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[macro_export]
macro_rules! register_module {
    ($module_type:ty) => {
        inventory::submit! {
            Box::new($module_type) as Box<dyn $crate::Module>
        }
    };
}
