#[macro_export]
macro_rules! builder_methods {
    ($( $method:ident ( $field:ident ) -> $ty:ty ),* $(,)?) => {
        $(
            pub fn $method(mut self, $field: $ty) -> Self {
                self.$field = Some($field);
                self
            }
        )*
    };
}
