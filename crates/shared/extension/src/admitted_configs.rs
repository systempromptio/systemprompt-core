//! Every admitted member's declared configuration sections, stamped with
//! their owners in composition order.
//!
//! The kernel builds one [`AdmittedConfigs`] from the admitted composition
//! and seeds it into state, open: a declaration is a section prefix, its
//! schema and its validator, never a value, so a member's command (`cloud
//! doctor`'s extension-config check) validates the services tree against it
//! without naming the admitted composition.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ExtensionId;

use crate::admitted::BuiltState;
use crate::contribute::ConfigDecl;

/// The owner-stamped configuration declarations of an admitted composition.
#[derive(Debug, Clone, Default)]
pub struct AdmittedConfigs {
    decls: Vec<(ExtensionId, ConfigDecl)>,
}

impl AdmittedConfigs {
    #[must_use]
    pub fn of(built: &BuiltState) -> Self {
        let decls = built
            .members()
            .flat_map(|member| {
                member
                    .sink
                    .configs()
                    .iter()
                    .map(|decl| (member.owner.clone(), *decl))
            })
            .collect();
        Self { decls }
    }

    #[must_use]
    pub fn decls(&self) -> &[(ExtensionId, ConfigDecl)] {
        &self.decls
    }
}
