#[macro_export]
macro_rules! register_module {
    ($module_type:ty) => {
        inventory::submit! {
            Box::new($module_type) as Box<dyn $crate::Module>
        }
    };
}
