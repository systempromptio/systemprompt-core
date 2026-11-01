use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use systemprompt_extension::notifications::{NotificationSink, NotificationSinkError};
use systemprompt_identifiers::UserId;
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_models::notifications::NotificationEvent;
use systemprompt_traits::{Dependencies, Job, JobContext, JobResult};

use crate::harness::Harness;

#[derive(Default)]
pub(crate) struct RecordedNotices {
    events: Mutex<Vec<NotificationEvent>>,
}

impl RecordedNotices {
    pub(crate) fn kinds(&self) -> Vec<String> {
        self.events
            .lock()
            .expect("lock")
            .iter()
            .map(|e| e.kind.as_str().to_owned())
            .collect()
    }
}

#[async_trait]
impl NotificationSink for RecordedNotices {
    fn is_available(&self) -> bool {
        true
    }

    async fn emit(
        &self,
        _conn: &mut sqlx::PgConnection,
        event: &NotificationEvent,
    ) -> Result<(), NotificationSinkError> {
        self.events.lock().expect("lock").push(event.clone());
        Ok(())
    }
}

pub(crate) async fn run(
    h: &Harness,
    job: &dyn Job,
    profile: BrainProfile,
    notices: Option<Arc<RecordedNotices>>,
) -> JobResult {
    let mut deps = Dependencies::new()
        .with(Arc::new(profile))
        .with(Arc::clone(&h.pool));
    if let Some(notices) = notices {
        let sink: systemprompt_extension::notifications::DynNotificationSink = notices;
        deps.insert(sink);
    }
    let ctx = JobContext::new(
        systemprompt_identifiers::Actor::user(UserId::generate()),
        deps,
    );
    job.execute(&ctx).await.expect("job runs")
}

pub(crate) async fn exec(h: &Harness, sql: &'static str, bind: &str) {
    sqlx::query(sql)
        .bind(bind)
        .execute(&h.pg)
        .await
        .expect("seed");
}
