//! Which provider a sealed value's reader is checked against, and whether an
//! install-scoped resolution withholds it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::TypeId;

use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::DependencyProvider;

use crate::reserved::{CoreId, install_reads};

pub(super) fn provider_id(provider: Option<&DependencyProvider>) -> Option<&str> {
    match provider {
        Some(DependencyProvider::Extension(provider)) => Some(provider.as_str()),
        Some(DependencyProvider::Host) => Some(CoreId::Kernel.id()),
        None => None,
    }
}

pub(super) fn install_withholds<T: Send + Sync + 'static>(
    provider: Option<&DependencyProvider>,
    owner: &ExtensionId,
) -> bool {
    match provider {
        Some(DependencyProvider::Extension(provider)) => provider != owner,
        _ => !install_reads(TypeId::of::<T>()),
    }
}
