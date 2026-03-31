/// Generates builder-style `with_*` methods for Option fields.
///
/// Each method takes `self` by value and returns `Self`, setting the field to `Some(value)`.
///
/// # Usage
/// ```rust
/// impl MyStruct {
///     builder_methods! {
///         with_name(name) -> String,
///         with_status(status) -> String,
///         with_limit(limit) -> i64,
///     }
/// }
/// ```
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
