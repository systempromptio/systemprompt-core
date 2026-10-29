//! [`BatchContext`]: what every fact of one drain round shares, loaded once
//! per round: the declared streams, the global rules, each stream's
//! subscriptions (on first use) and the membership of each rule subject (on
//! first use).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, HashSet};

use sqlx::PgConnection;
use systemprompt_identifiers::{NotificationStreamId, UserId};
use systemprompt_models::notifications::{AudienceSubject, RuleDecl, StreamDecl};

use super::membership::Members;
use super::withheld::Withheld;
use crate::error::NotificationsResult;
use crate::models::SubscriptionView;
use crate::repository::{
    NotificationRuleRepository, NotificationStreamRepository, NotificationSubscriptionRepository,
};

/// Declarations and memberships shared by every fact of one drain round.
#[derive(Debug, Default)]
pub struct BatchContext {
    streams: HashMap<NotificationStreamId, StreamDecl>,
    global_rules: Vec<RuleDecl>,
    subscriptions: HashMap<NotificationStreamId, Vec<SubscriptionView>>,
    rule_members: HashMap<AudienceSubject, Result<HashSet<UserId>, Withheld>>,
}

impl BatchContext {
    pub async fn load(conn: &mut PgConnection) -> NotificationsResult<Self> {
        let streams = NotificationStreamRepository::list(&mut *conn)
            .await?
            .into_iter()
            .map(|stream| (stream.id.clone(), stream))
            .collect();
        Ok(Self {
            streams,
            global_rules: NotificationRuleRepository::list_global(&mut *conn).await?,
            subscriptions: HashMap::new(),
            rule_members: HashMap::new(),
        })
    }

    pub(crate) fn stream(&self, id: &NotificationStreamId) -> Option<&StreamDecl> {
        self.streams.get(id)
    }

    pub(crate) fn global_rules(&self) -> &[RuleDecl] {
        &self.global_rules
    }

    pub(crate) async fn stream_subscriptions(
        &mut self,
        conn: &mut PgConnection,
        id: &NotificationStreamId,
    ) -> NotificationsResult<Vec<SubscriptionView>> {
        if let Some(views) = self.subscriptions.get(id) {
            return Ok(views.clone());
        }
        let views = NotificationSubscriptionRepository::list_for_stream(&mut *conn, id).await?;
        self.subscriptions.insert(id.clone(), views.clone());
        Ok(views)
    }

    pub(crate) async fn rule_members(
        &mut self,
        members: &Members<'_>,
        subject: &AudienceSubject,
    ) -> Result<HashSet<UserId>, Withheld> {
        if let Some(known) = self.rule_members.get(subject) {
            return known.clone();
        }
        let resolved = members
            .expand(subject)
            .await
            .map(|users| users.into_iter().collect::<HashSet<_>>());
        self.rule_members.insert(subject.clone(), resolved.clone());
        resolved
    }
}
