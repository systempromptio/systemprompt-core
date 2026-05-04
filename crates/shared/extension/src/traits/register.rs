/// Registers an extension implementation with the global `inventory`
/// collector.
///
/// Two forms are supported:
///
/// ```ignore
/// // Type-only form (extension type must implement `Default`).
/// register_extension!(MyExtension);
///
/// // Expression form (used when construction needs arguments).
/// register_extension!(MyExtension::with_config(cfg));
/// ```
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
