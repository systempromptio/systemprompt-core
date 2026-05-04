//! Heterogeneous-list type machinery used by the extension builder's
//! dependency typestate.

use std::any::TypeId;

/// Operations on a heterogeneous list of types.
pub trait TypeList: 'static {
    /// Returns true if `T` is contained anywhere in the list.
    fn contains_type<T: 'static>() -> bool;
    /// Returns the `TypeId` of every element in the list, head-first.
    fn type_ids() -> Vec<TypeId>;
    /// Returns the number of elements in the list.
    fn len() -> usize;
    /// Returns true if the list is empty.
    fn is_empty() -> bool {
        Self::len() == 0
    }
}

impl TypeList for () {
    fn contains_type<T: 'static>() -> bool {
        false
    }

    fn type_ids() -> Vec<TypeId> {
        vec![]
    }

    fn len() -> usize {
        0
    }
}

impl<H: 'static, T: TypeList> TypeList for (H, T) {
    fn contains_type<X: 'static>() -> bool {
        TypeId::of::<H>() == TypeId::of::<X>() || T::contains_type::<X>()
    }

    fn type_ids() -> Vec<TypeId> {
        let mut ids = vec![TypeId::of::<H>()];
        ids.extend(T::type_ids());
        ids
    }

    fn len() -> usize {
        1 + T::len()
    }
}

/// Marker witnessed when one type list is a subset of another.
pub trait Subset<B: TypeList>: TypeList {
    /// Returns true if every element of `Self` appears in `B`.
    fn is_subset_of() -> bool;
}

impl<B: TypeList> Subset<B> for () {
    fn is_subset_of() -> bool {
        true
    }
}

impl<H: 'static, T: TypeList + Subset<B>, B: TypeList> Subset<B> for (H, T) {
    fn is_subset_of() -> bool {
        B::contains_type::<H>() && T::is_subset_of()
    }
}

/// Marker witnessed when a type list contains a specific type.
pub trait Contains<T: 'static>: TypeList {}

impl<T: 'static, Tail: TypeList> Contains<T> for (T, Tail) {}

/// Marker witnessed when two types are not the same. Used by typestate
/// builders to forbid double-registration.
pub trait NotSame {}

impl<A, B> NotSame for (A, B) {}
