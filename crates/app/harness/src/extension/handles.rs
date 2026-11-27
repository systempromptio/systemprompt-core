//! Values an extension shares with its shell half and with other extensions,
//! found again by their type.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use parking_lot::RwLock;

type Entry = Arc<dyn Any + Send + Sync>;

/// Handles inserted once at startup and found again by their type; cheap to
/// clone, every clone sees the same entries.
#[derive(Clone, Default)]
pub struct Handles {
    entries: Arc<RwLock<HashMap<TypeId, Entry>>>,
}

impl Handles {
    pub fn insert<T: Send + Sync + 'static>(&self, value: Arc<T>) {
        let entry: Entry = value;
        self.entries.write().insert(TypeId::of::<T>(), entry);
    }

    pub fn get<T: Send + Sync + 'static>(&self) -> Option<Arc<T>> {
        let entry = self.entries.read().get(&TypeId::of::<T>()).cloned()?;
        entry.downcast::<T>().ok()
    }

    pub fn contains<T: Send + Sync + 'static>(&self) -> bool {
        self.entries.read().contains_key(&TypeId::of::<T>())
    }
}

impl fmt::Debug for Handles {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Handles")
            .field("entries", &self.entries.read().len())
            .finish()
    }
}
