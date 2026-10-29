//! A user's own rules, applied after the declared ones to that user's
//! notifications only. Setting them replaces the whole list; an id a
//! declared rule holds is refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use systemprompt_extension::prelude::ChangeSource;
use systemprompt_identifiers::UserId;
use systemprompt_models::notifications::{DeclError, RuleDecl};

use super::NotificationsService;
use crate::error::{NotificationsError, NotificationsResult};

impl NotificationsService {
    pub async fn own_rules(&self, user: &UserId) -> NotificationsResult<Vec<RuleDecl>> {
        Ok(self.repos.rules.list_for_owner(user).await?)
    }

    pub async fn set_own_rules(
        &self,
        user: &UserId,
        rules: Vec<RuleDecl>,
    ) -> NotificationsResult<()> {
        let mut seen = HashSet::new();
        for rule in &rules {
            if !seen.insert(&rule.id) {
                return Err(DeclError::DuplicateRule(rule.id.clone()).into());
            }
            if let (Some(floor), Some(cap)) = (rule.action.floor, rule.action.cap)
                && floor > cap
            {
                return Err(DeclError::RuleFloorAboveCap(rule.id.clone()).into());
            }
        }
        let source = ChangeSource::Console(user.clone()).to_string();
        if let Some(id) = self
            .repos
            .rules
            .replace_owner_rules(user, rules, &source)
            .await?
        {
            return Err(NotificationsError::DeclaredByConfig {
                entity: "notification_rule",
                key: id.to_string(),
            });
        }
        Ok(())
    }
}
