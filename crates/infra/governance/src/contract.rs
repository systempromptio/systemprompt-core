//! The capability slots governance owns.
//!
//! [`AuthzHookSlot`] collects the in-process authz decision hooks members
//! contribute; [`RoleDirectorySlot`] collects the existence oracle for the
//! `role` subject dimension. Both are [`SlotOrder::CoreFirst`]: a core
//! member's entry is always consulted before any install's, whatever the
//! composition order.
//!
//! ```ignore
//! c.slot::<AuthzHookSlot>().hook(|r: &Resolved<'_>| Ok(Arc::new(MyHook::new(r.get()?)) as SharedAuthzHook));
//! c.slot::<RoleDirectorySlot>().directory(|r: &Resolved<'_>| Ok(Arc::new(MyRoles::new(r.get()?)) as SharedRoleDirectory));
//! ```
//!
//! Consumers read the built values with `r.get::<Arc<AuthzHooks>>()` and
//! `r.get::<Arc<RoleDirectories>>()`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::{
    Factory, FactoryError, Resolved, Slot, SlotEntry, SlotError, SlotOrder,
};
use systemprompt_identifiers::ExtensionId;

use crate::authz::{CompositeAuthzHook, SharedAuthzHook, SharedRoleDirectory};

const AUTHZ_HOOK_SLOT: &str = "authz_hook";
const ROLE_DIRECTORY_SLOT: &str = "role_directory";

type HookFactory = Box<dyn Factory<SharedAuthzHook>>;
type DirectoryFactory = Box<dyn Factory<SharedRoleDirectory>>;

fn entry_error(slot: &'static str, owner: &ExtensionId, source: FactoryError) -> SlotError {
    match source {
        FactoryError::Missing(missing) => SlotError::Missing(missing),
        FactoryError::Failed(source) => SlotError::Entry {
            slot,
            owner: owner.clone(),
            source,
        },
    }
}

/// The in-process authz decision hooks members contribute.
#[derive(Debug, Clone, Copy)]
pub struct AuthzHookSlot;

#[derive(Default)]
pub struct AuthzHookBuilder {
    hooks: Vec<HookFactory>,
}

impl std::fmt::Debug for AuthzHookBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthzHookBuilder")
            .field("hooks", &self.hooks.len())
            .finish()
    }
}

impl AuthzHookBuilder {
    pub fn hook<F: Factory<SharedAuthzHook>>(&mut self, f: F) -> &mut Self {
        self.hooks.push(Box::new(f));
        self
    }
}

/// The contributed authz hooks, composed core first into one hook.
#[derive(Debug, Clone, Default)]
pub struct AuthzHooks {
    hook: Option<SharedAuthzHook>,
}

impl AuthzHooks {
    #[must_use]
    pub fn hook(&self) -> Option<SharedAuthzHook> {
        self.hook.clone()
    }
}

impl Slot for AuthzHookSlot {
    type Builder = AuthzHookBuilder;
    type Built = AuthzHooks;
    const ORDER: SlotOrder = SlotOrder::CoreFirst;

    fn build(
        entries: Vec<SlotEntry<Self::Builder>>,
        r: &Resolved<'_>,
    ) -> Result<Self::Built, SlotError> {
        let mut hooks = Vec::new();
        for entry in entries {
            for factory in entry.builder.hooks {
                hooks.push(
                    factory
                        .build(r)
                        .map_err(|e| entry_error(AUTHZ_HOOK_SLOT, &entry.owner, e))?,
                );
            }
        }
        let mut hooks = hooks.into_iter();
        let hook = hooks.next().map(|first| {
            let rest: Vec<SharedAuthzHook> = hooks.collect();
            if rest.is_empty() {
                first
            } else {
                Arc::new(CompositeAuthzHook::new(first, rest))
            }
        });
        Ok(AuthzHooks { hook })
    }
}

/// The existence oracle for the `role` subject dimension. Single-valued: a
/// second contributed directory is refused.
#[derive(Debug, Clone, Copy)]
pub struct RoleDirectorySlot;

#[derive(Default)]
pub struct RoleDirectoryBuilder {
    directories: Vec<DirectoryFactory>,
}

impl std::fmt::Debug for RoleDirectoryBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RoleDirectoryBuilder")
            .field("directories", &self.directories.len())
            .finish()
    }
}

impl RoleDirectoryBuilder {
    pub fn directory<F: Factory<SharedRoleDirectory>>(&mut self, f: F) -> &mut Self {
        self.directories.push(Box::new(f));
        self
    }
}

/// The contributed role directory, when a member contributed one.
#[derive(Debug, Clone, Default)]
pub struct RoleDirectories {
    directory: Option<SharedRoleDirectory>,
}

impl RoleDirectories {
    #[must_use]
    pub fn directory(&self) -> Option<SharedRoleDirectory> {
        self.directory.clone()
    }
}

impl Slot for RoleDirectorySlot {
    type Builder = RoleDirectoryBuilder;
    type Built = RoleDirectories;
    const ORDER: SlotOrder = SlotOrder::CoreFirst;

    fn build(
        entries: Vec<SlotEntry<Self::Builder>>,
        r: &Resolved<'_>,
    ) -> Result<Self::Built, SlotError> {
        let count = entries.iter().map(|e| e.builder.directories.len()).sum();
        if count > 1 {
            return Err(SlotError::MultipleEntries {
                slot: ROLE_DIRECTORY_SLOT,
                count,
            });
        }
        let mut directory = None;
        for entry in entries {
            for factory in entry.builder.directories {
                directory = Some(
                    factory
                        .build(r)
                        .map_err(|e| entry_error(ROLE_DIRECTORY_SLOT, &entry.owner, e))?,
                );
            }
        }
        Ok(RoleDirectories { directory })
    }
}
