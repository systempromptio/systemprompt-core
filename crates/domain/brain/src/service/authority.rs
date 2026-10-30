//! Who may act on a fact, decided fail-closed.
//!
//! A user-scoped fact belongs to its owner alone. A project, group or org
//! fact may be governed (confirmed, retracted, erased, pinned, edited, its
//! disputes resolved) by an admin or by a caller holding the
//! `confirm_role` of a workflow profile that lists the fact's kind; when no
//! such role is declared and the caller is not an admin, the action is
//! refused. Viewing and disputing need only that a personal fact is the
//! caller's own.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{FactKindId, UserId};
use systemprompt_models::auth::BaseRoles;
use systemprompt_models::brain::{FactScope, WorkflowProfileDecl};

use crate::error::{BrainError, BrainResult};

/// The person acting, with the roles their token carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrainCaller {
    pub user_id: UserId,
    pub roles: Vec<String>,
}

impl BrainCaller {
    #[must_use]
    pub const fn new(user_id: UserId, roles: Vec<String>) -> Self {
        Self { user_id, roles }
    }

    #[must_use]
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|held| held == role)
    }

    #[must_use]
    pub fn is_admin(&self) -> bool {
        self.has_role(BaseRoles::ADMIN)
    }
}

fn refused(reason: &str) -> BrainError {
    BrainError::Forbidden {
        reason: reason.to_owned(),
    }
}

pub fn may_govern(
    caller: &BrainCaller,
    scope: &FactScope,
    kind: &FactKindId,
    workflows: &[WorkflowProfileDecl],
) -> BrainResult<()> {
    if let FactScope::User(owner) = scope {
        return if owner == &caller.user_id {
            Ok(())
        } else {
            Err(refused("only its owner may act on a personal fact"))
        };
    }
    if caller.is_admin() {
        return Ok(());
    }
    let named = workflows
        .iter()
        .filter(|workflow| workflow.kinds.contains(kind))
        .filter_map(|workflow| workflow.confirm_role.as_deref())
        .any(|role| caller.has_role(role));
    if named {
        Ok(())
    } else {
        Err(refused(
            "the caller holds no role a workflow profile names for this kind",
        ))
    }
}

pub fn may_view(caller: &BrainCaller, scope: &FactScope) -> BrainResult<()> {
    match scope {
        FactScope::User(owner) if owner != &caller.user_id => {
            Err(refused("a personal fact is visible to its owner only"))
        },
        _ => Ok(()),
    }
}

pub fn may_enter(caller: &UserId, scope: &FactScope) -> BrainResult<()> {
    match scope {
        FactScope::User(owner) if owner != caller => {
            Err(refused("a personal fact may only be entered by its owner"))
        },
        _ => Ok(()),
    }
}

pub fn may_decide_proposals(caller: &BrainCaller) -> BrainResult<()> {
    if caller.is_admin() {
        Ok(())
    } else {
        Err(refused("only an admin may decide a proposal"))
    }
}
