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
