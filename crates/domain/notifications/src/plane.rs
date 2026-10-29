//! The `notifications` config plane: `services/notifications/streams.yaml`,
//! `rules.yaml` and `subscriptions.yaml`, merged into one
//! [`NotificationsDoc`] and projected into `notification_streams`, the
//! ownerless `notification_rules` and the `code:`-sourced
//! `notification_subscriptions`.
//!
//! Entities are keyed `stream/<id>`, `rule/<id>` and
//! `subscription/<subject>|stream:<id>` or
//! `subscription/<subject>|resource:<kind>:<id>`; each value is the
//! declaration's canonical JSON, and each declared entity's origin is the
//! file that declares it. A user's own rules and subscriptions are runtime
//! state and never enter the plane: the plane never adopts, overwrites or
//! deletes them, and a declaration colliding with one is refused. The files
//! are absent-tolerant: no file at all leaves the database authoritative.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::Path;

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use systemprompt_database::PlaneTransaction;
use systemprompt_extension::prelude::{
    ApplyOutcome, ChangeSource, ConfigPlane, ConfigPlaneError, ConfigPlaneResult, DeclaredEntities,
    DeclaredEntity, DeclaredState, PlaneContext, PlaneSnapshot, PlaneTx, ResolvedEntities,
};
use systemprompt_identifiers::{NotificationRuleId, NotificationStreamId};
use systemprompt_manifest::services::notifications::{
    NOTIFICATIONS_DIR, RULES_FILE, STREAMS_FILE, SUBSCRIPTIONS_FILE,
};
use systemprompt_models::notifications::{
    NotificationsDoc, RuleDecl, StreamDecl, SubscriptionDecl, SubscriptionTarget, validate,
};

use crate::repository::{
    NotificationRuleRepository, NotificationStreamRepository, NotificationSubscriptionRepository,
};

pub const NOTIFICATIONS_PLANE: &str = "notifications";

const FILES: [&str; 3] = [STREAMS_FILE, RULES_FILE, SUBSCRIPTIONS_FILE];

/// The notifications declarations as a config plane.
#[derive(Debug, Clone, Copy, Default)]
pub struct NotificationsPlane;

type Sourced = BTreeMap<String, (String, String)>;

impl NotificationsPlane {
    fn declared(services_root: &Path) -> ConfigPlaneResult<Option<Sourced>> {
        let mut merged = NotificationsDoc::default();
        let mut origins: Vec<(String, NotificationsDoc)> = Vec::new();
        for file in FILES {
            let relative = format!("{NOTIFICATIONS_DIR}/{file}");
            let path = services_root.join(&relative);
            if !path.exists() {
                continue;
            }
            let text = std::fs::read_to_string(&path).map_err(|e| unreadable(&relative, e))?;
            let doc: NotificationsDoc =
                serde_yaml::from_str(&text).map_err(|e| unreadable(&relative, e))?;
            merged.merge(doc.clone());
            origins.push((relative, doc));
        }
        if origins.is_empty() {
            return Ok(None);
        }
        validate(&merged).map_err(|e| invalid(NOTIFICATIONS_DIR, &e))?;
        let mut entities = Sourced::new();
        for (file, doc) in origins {
            for (key, value) in doc_entities(&doc).map_err(|e| invalid(&file, &e))? {
                if entities
                    .insert(key.clone(), (value, file.clone()))
                    .is_some()
                {
                    return Err(invalid(&file, &format!("{key} is declared more than once")));
                }
            }
        }
        Ok(Some(entities))
    }
}

fn doc_entities(doc: &NotificationsDoc) -> Result<Vec<(String, String)>, serde_json::Error> {
    let mut out = Vec::new();
    for stream in &doc.streams {
        out.push((
            format!("stream/{}", stream.id),
            serde_json::to_string(stream)?,
        ));
    }
    for rule in &doc.rules {
        out.push((format!("rule/{}", rule.id), serde_json::to_string(rule)?));
    }
    for subscription in &doc.subscriptions {
        out.push((
            subscription_key(subscription),
            serde_json::to_string(subscription)?,
        ));
    }
    Ok(out)
}

fn subscription_key(decl: &SubscriptionDecl) -> String {
    match &decl.target {
        SubscriptionTarget::Stream(stream) => {
            format!("subscription/{}|stream:{stream}", decl.subject)
        },
        SubscriptionTarget::Resource(r) => {
            format!("subscription/{}|resource:{}:{}", decl.subject, r.kind, r.id)
        },
    }
}

fn unreadable(
    path: &str,
    error: impl std::error::Error + Send + Sync + 'static,
) -> ConfigPlaneError {
    ConfigPlaneError::Unreadable {
        plane: NOTIFICATIONS_PLANE.to_owned(),
        path: path.to_owned(),
        source: Box::new(error),
    }
}

fn invalid(path: &str, message: &dyn std::fmt::Display) -> ConfigPlaneError {
    ConfigPlaneError::Invalid {
        plane: NOTIFICATIONS_PLANE.to_owned(),
        path: path.to_owned(),
        message: message.to_string(),
    }
}

fn backend(error: impl std::error::Error + Send + Sync + 'static) -> ConfigPlaneError {
    ConfigPlaneError::Backend {
        plane: NOTIFICATIONS_PLANE.to_owned(),
        source: Box::new(error),
    }
}

fn decode<T: DeserializeOwned>(key: &str, value: &str) -> ConfigPlaneResult<T> {
    serde_json::from_str(value).map_err(|e| invalid(key, &e))
}

#[async_trait]
impl ConfigPlane for NotificationsPlane {
    fn name(&self) -> &'static str {
        NOTIFICATIONS_PLANE
    }

    fn declared_path(&self) -> &'static str {
        NOTIFICATIONS_DIR
    }

    fn load_declared(&self, services_root: &Path) -> ConfigPlaneResult<Option<DeclaredState>> {
        Ok(Self::declared(services_root)?.map(|entities| {
            DeclaredState::from_entries(entities.into_iter().map(|(k, (v, _))| (k, v)).collect())
        }))
    }

    fn sources(&self, services_root: &Path) -> ConfigPlaneResult<Option<DeclaredEntities>> {
        Ok(Self::declared(services_root)?.map(|entities| {
            entities
                .into_iter()
                .map(|(key, (value, file))| {
                    (key, DeclaredEntity::new(value, ChangeSource::Code(file)))
                })
                .collect()
        }))
    }

    async fn read_current(
        &self,
        _ctx: &PlaneContext<'_>,
        tx: &mut PlaneTx<'_>,
    ) -> ConfigPlaneResult<PlaneSnapshot> {
        let tx = tx.get::<PlaneTransaction>()?;
        let doc = NotificationsDoc {
            streams: NotificationStreamRepository::list(&mut **tx)
                .await
                .map_err(backend)?,
            rules: NotificationRuleRepository::list_global(&mut **tx)
                .await
                .map_err(backend)?,
            subscriptions: NotificationSubscriptionRepository::list_declared(&mut **tx)
                .await
                .map_err(backend)?
                .into_iter()
                .map(|view| view.decl)
                .collect(),
        };
        Ok(doc_entities(&doc)
            .map_err(|e| invalid(NOTIFICATIONS_DIR, &e))?
            .into_iter()
            .collect())
    }

    async fn write(
        &self,
        ctx: &PlaneContext<'_>,
        tx: &mut PlaneTx<'_>,
        entities: &ResolvedEntities,
    ) -> ConfigPlaneResult<ApplyOutcome> {
        let current = self.read_current(ctx, tx).await?;
        let tx = tx.get::<PlaneTransaction>()?;
        let mut outcome = ApplyOutcome::default();
        for (key, resolved) in entities {
            let source = resolved.source.to_string();
            let value = resolved.value.as_deref();
            let (kind, id) = key
                .split_once('/')
                .ok_or_else(|| invalid(key, &"unknown entity key"))?;
            match (kind, value) {
                ("stream", Some(v)) => {
                    let stream: StreamDecl = decode(key, v)?;
                    NotificationStreamRepository::upsert(&mut **tx, &stream, &source)
                        .await
                        .map_err(backend)?;
                },
                ("stream", None) => {
                    let id = NotificationStreamId::try_new(id).map_err(|e| invalid(key, &e))?;
                    NotificationStreamRepository::delete(&mut **tx, &id)
                        .await
                        .map_err(backend)?;
                },
                ("rule", Some(v)) => {
                    let rule: RuleDecl = decode(key, v)?;
                    NotificationRuleRepository::upsert_declared(&mut **tx, &rule, &source)
                        .await
                        .map_err(backend)?;
                },
                ("rule", None) => {
                    let id = NotificationRuleId::try_new(id).map_err(|e| invalid(key, &e))?;
                    NotificationRuleRepository::delete_declared(&mut **tx, &id)
                        .await
                        .map_err(backend)?;
                },
                ("subscription", Some(v)) => {
                    let decl: SubscriptionDecl = decode(key, v)?;
                    NotificationSubscriptionRepository::upsert(&mut **tx, &decl, &source, None)
                        .await
                        .map_err(backend)?
                        .ok_or_else(|| {
                            invalid(key, &"a user's own subscription holds this target")
                        })?;
                },
                ("subscription", None) => {
                    let stored = current
                        .get(key)
                        .ok_or_else(|| invalid(key, &"not stored"))?;
                    let decl: SubscriptionDecl = decode(key, stored)?;
                    NotificationSubscriptionRepository::delete_declared(&mut **tx, &decl)
                        .await
                        .map_err(backend)?;
                },
                _ => return Err(invalid(key, &"unknown entity kind")),
            }
            if value.is_some() {
                outcome.updated += 1;
            } else {
                outcome.deleted += 1;
            }
        }
        Ok(outcome)
    }

    async fn export(
        &self,
        ctx: &PlaneContext<'_>,
        tx: &mut PlaneTx<'_>,
    ) -> ConfigPlaneResult<Option<String>> {
        let current = self.read_current(ctx, tx).await?;
        if current.is_empty() {
            return Ok(None);
        }
        self.render_declarations(&current).map(Some)
    }

    fn render_declarations(&self, entities: &PlaneSnapshot) -> ConfigPlaneResult<String> {
        let mut doc = NotificationsDoc::default();
        for (key, value) in entities {
            match key.split_once('/').map(|(kind, _)| kind) {
                Some("stream") => doc.streams.push(decode(key, value)?),
                Some("rule") => doc.rules.push(decode(key, value)?),
                Some("subscription") => doc.subscriptions.push(decode(key, value)?),
                _ => return Err(invalid(key, &"unknown entity kind")),
            }
        }
        serde_yaml::to_string(&doc).map_err(|e| invalid(NOTIFICATIONS_DIR, &e))
    }
}
