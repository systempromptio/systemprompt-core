//! [`CoreSeam`]: a core default seam one domain provides for a slot another
//! domain owns.
//!
//! A core default that reads one domain and serves another (the user-role
//! audience resolver users owns, the document and issue visibilities and the
//! document source access and content files owns) is provided by its owning
//! member into the extension state as a `CoreSeam<dyn Trait, Kind>`. The
//! consuming member's slot reads it with `r.optional` and appends it after
//! the contributed seams, so a disabled owner leaves the default absent and
//! neither domain names the other. The kind marker keeps two defaults of one
//! trait (the document and the issue visibility) distinct state entries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

/// One core default seam of type `T`, keyed in the extension state by its
/// kind marker `K`.
pub struct CoreSeam<T: ?Sized, K> {
    seam: Arc<T>,
    kind: PhantomData<fn() -> K>,
}

impl<T: ?Sized, K> CoreSeam<T, K> {
    #[must_use]
    pub const fn new(seam: Arc<T>) -> Self {
        Self {
            seam,
            kind: PhantomData,
        }
    }

    #[must_use]
    pub fn seam(&self) -> Arc<T> {
        Arc::clone(&self.seam)
    }
}

impl<T: ?Sized, K> Clone for CoreSeam<T, K> {
    fn clone(&self) -> Self {
        Self::new(Arc::clone(&self.seam))
    }
}

impl<T: ?Sized, K> fmt::Debug for CoreSeam<T, K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("CoreSeam")
            .field(&std::any::type_name::<K>())
            .finish()
    }
}

/// The user-role audience resolver the `users` member provides.
#[derive(Debug, Clone, Copy)]
pub enum RoleAudience {}

/// The document, folder and comment visibility the `files` member provides.
#[derive(Debug, Clone, Copy)]
pub enum DocumentVisibility {}

/// The issue-report visibility the `files` member provides.
#[derive(Debug, Clone, Copy)]
pub enum IssueVisibility {}

/// The document-sourced fact access the `files` member provides.
#[derive(Debug, Clone, Copy)]
pub enum DocumentSourceAccess {}

/// The document and comment source reader the `files` member provides.
#[derive(Debug, Clone, Copy)]
pub enum DocumentSourceContent {}
