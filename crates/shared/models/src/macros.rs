//! Declarative macros shared across the crate's builder types.
//!
//! `builder_methods!` generates fluent setter methods that wrap each
//! field value in `Some(..)`, for builders over structs whose fields
//! are `Option<T>`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
