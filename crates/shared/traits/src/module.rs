//! Compile-time module registration macro.

/// Register a runtime [`Module`](crate::Module) implementation with
/// `inventory` so the binary can discover it during startup.
///
/// # Examples
///
/// ```ignore
/// systemprompt_traits::register_module!(MyModule);
/// ```
#[macro_export]
macro_rules! register_module {
    ($module_type:ty) => {
        inventory::submit! {
            Box::new($module_type) as Box<dyn $crate::Module>
        }
    };
}
