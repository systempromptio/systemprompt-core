use std::any::TypeId;

pub trait TypeList: 'static {
    fn contains_type<T: 'static>() -> bool;
    fn type_ids() -> Vec<TypeId>;
    fn len() -> usize;
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

pub trait Subset<B: TypeList>: TypeList {
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

pub trait Contains<T: 'static>: TypeList {}

impl<T: 'static, Tail: TypeList> Contains<T> for (T, Tail) {}

pub trait NotSame {}

impl<A, B> NotSame for (A, B) {}
