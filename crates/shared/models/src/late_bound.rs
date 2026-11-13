//! [`LateBound`]: the one bind-once cell for a value composed after the
//! things that read it are built.
//!
//! The host creates the cell first, hands clones to every reader, and binds
//! it once the value exists. Clones share the binding. A read before binding
//! is `None`, which every wrapper turns into its own typed not-ready error,
//! never an empty default; a second bind is refused with [`AlreadyBound`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::{Arc, OnceLock};

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("the late-bound value is already bound")]
pub struct AlreadyBound;

/// A value bound once, after its readers exist; see the module docs.
pub struct LateBound<T: ?Sized>(Arc<OnceLock<Arc<T>>>);

impl<T: ?Sized> LateBound<T> {
    #[must_use]
    pub fn bound(value: Arc<T>) -> Self {
        Self(Arc::new(OnceLock::from(value)))
    }

    pub fn bind(&self, value: Arc<T>) -> Result<(), AlreadyBound> {
        self.0.set(value).map_err(|_bound| AlreadyBound)
    }

    #[must_use]
    pub fn get(&self) -> Option<&Arc<T>> {
        self.0.get()
    }

    #[must_use]
    pub fn is_bound(&self) -> bool {
        self.0.get().is_some()
    }
}

impl<T: ?Sized> Clone for LateBound<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<T: ?Sized> Default for LateBound<T> {
    fn default() -> Self {
        Self(Arc::new(OnceLock::new()))
    }
}

impl<T: ?Sized> fmt::Debug for LateBound<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LateBound")
            .field("bound", &self.is_bound())
            .finish()
    }
}
