//! The `licence_checkin` job: a report-only licence check-in for
//! multi-tenant deployments.
//!
//! When `licence.multi_tenant` is set, one replica (cluster-scoped claim)
//! posts a [`LicenceCheckin`] to `licence.endpoint` at most once per
//! `licence.interval_secs`: the licence and tenant ids, this instance, the
//! core version, the hosting provider, the external URL host and the live
//! replica count per `server.role`. The body is signed with HMAC-SHA256 under
//! the `licence_signing_secret` secret, hex-encoded in
//! [`SIGNATURE_HEADER`]. The check-in never gates anything: a failure is
//! logged and recorded in `licence_checkin_state` and the job still reports
//! success, so the platform's behaviour is the same whatever the endpoint
//! answers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use systemprompt_client::{GuardedClientConfig, guarded_client};
use systemprompt_config::LoadedProfile;
use systemprompt_database::{DbPool, RoleCount};
use systemprompt_identifiers::InstanceId;
use systemprompt_manifest::profile::{LICENCE_SIGNING_SECRET, LicenceConfig, Profile};
use systemprompt_models::licence::{
    CHECKIN_SIGNATURE_HEADER, CHECKIN_TENANT_HEADER, LicenceCheckin, sign_checkin,
};
use systemprompt_models::net::validate_outbound_url;
use systemprompt_provider_contracts::Dependencies;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};
use tracing::{debug, info, warn};

use crate::repository::LicenceCheckinRepository;
use crate::services::scheduling::job_app_context;

pub const JOB_NAME: &str = "licence_checkin";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

#[must_use]
pub fn build_checkin(
    profile: &Profile,
    instance_id: InstanceId,
    roles: &[RoleCount],
    timestamp: DateTime<Utc>,
) -> LicenceCheckin {
    let external_host = url::Url::parse(&profile.server.external_url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned));
    LicenceCheckin {
        licence_id: profile.licence.licence_id.clone(),
        tenant_id: profile.cloud.as_ref().and_then(|c| c.tenant_id.clone()),
        instance_id,
        core_version: env!("CARGO_PKG_VERSION").to_owned(),
        hosting_provider: profile.hosting_provider().as_str().to_owned(),
        external_host,
        instances_by_role: roles
            .iter()
            .map(|r| (r.role.clone(), r.instances))
            .collect(),
        timestamp,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LicenceCheckinError {
    #[error("licence_signing_secret is not set")]
    MissingSecret,
    #[error("licence check-in body could not be encoded: {0}")]
    Encode(#[source] serde_json::Error),
    #[error("licence endpoint refused: {0}")]
    Endpoint(#[source] systemprompt_models::net::OutboundUrlError),
    #[error("licence endpoint answered {status}")]
    Status { status: reqwest::StatusCode },
    #[error("licence check-in request failed: {0}")]
    Request(#[from] reqwest::Error),
}

impl LicenceCheckinError {
    fn status(&self) -> Option<u16> {
        match self {
            Self::Status { status } => Some(status.as_u16()),
            Self::Request(e) => e.status().map(|s| s.as_u16()),
            _ => None,
        }
    }
}

#[must_use]
pub fn checkin_due(
    config: &LicenceConfig,
    last_attempt_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> bool {
    let interval = i64::try_from(config.effective_interval_secs())
        .ok()
        .and_then(chrono::Duration::try_seconds)
        .unwrap_or(chrono::Duration::MAX);
    last_attempt_at.is_none_or(|last| now - last >= interval)
}

#[derive(Debug, Clone, Copy)]
pub struct LicenceCheckinJob;

#[async_trait]
impl Job for LicenceCheckinJob {
    fn name(&self) -> &'static str {
        JOB_NAME
    }

    fn description(&self) -> &'static str {
        "Reports a signed, report-only licence check-in for multi-tenant deployments"
    }

    fn schedule(&self) -> &'static str {
        "0 */5 * * * *"
    }

    // Why: a deployment without `licence.multi_tenant` never checks in, so the
    // job is not scheduled at all; the profile is fixed for the life of the
    // process.
    fn configured(&self, deployment: &Dependencies) -> bool {
        deployment
            .get::<Arc<LoadedProfile>>()
            .is_ok_and(|profile| profile.licence.enabled())
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let start = std::time::Instant::now();
        let app = job_app_context(ctx)?;
        let profile = app.profile();
        if !profile.licence.enabled() {
            debug!("licence.multi_tenant is off; no check-in");
            return Ok(JobResult::success().with_duration(start.elapsed().as_millis() as u64));
        }
        let db_pool = ctx.get::<DbPool>()?;
        let repository = LicenceCheckinRepository::new(db_pool.write_pool().as_ref().clone());
        let last_attempt = repository
            .find_state()
            .await?
            .and_then(|state| state.last_attempt_at);
        if !checkin_due(&profile.licence, last_attempt, Utc::now()) {
            return Ok(JobResult::success().with_duration(start.elapsed().as_millis() as u64));
        }
        repository.mark_attempt().await?;

        let roles = match app
            .service_repository()
            .count_live_instances_by_role()
            .await
        {
            Ok(roles) => roles,
            Err(error) => {
                warn!(error = %error, "licence check-in could not count live instances");
                Vec::new()
            },
        };
        let checkin = build_checkin(
            profile,
            app.config().instance_id.clone(),
            &roles,
            Utc::now(),
        );
        let secret = app.secrets().get().get(LICENCE_SIGNING_SECRET).cloned();
        match post_checkin(&profile.licence, secret.as_deref(), &checkin).await {
            Ok(status) => {
                info!(
                    endpoint = %profile.licence.endpoint,
                    status,
                    "licence check-in reported"
                );
                repository.record_success(status).await?;
            },
            Err(error) => {
                warn!(
                    endpoint = %profile.licence.endpoint,
                    status = ?error.status(),
                    error = %error,
                    "licence check-in failed; service is unaffected"
                );
                repository
                    .record_failure(error.status(), &error.to_string())
                    .await?;
            },
        }
        Ok(JobResult::success()
            .with_stats(1, 0)
            .with_duration(start.elapsed().as_millis() as u64))
    }
}

systemprompt_provider_contracts::submit_job!(&LicenceCheckinJob);

async fn post_checkin(
    config: &LicenceConfig,
    secret: Option<&str>,
    checkin: &LicenceCheckin,
) -> Result<u16, LicenceCheckinError> {
    let secret = secret.ok_or(LicenceCheckinError::MissingSecret)?;
    let url = validate_outbound_url(&config.endpoint).map_err(LicenceCheckinError::Endpoint)?;
    let body = serde_json::to_vec(checkin).map_err(LicenceCheckinError::Encode)?;
    let signature = sign_checkin(&body, secret.as_bytes());
    let client = guarded_client(&GuardedClientConfig::default().with_timeout(REQUEST_TIMEOUT))?;
    let mut request = client
        .post(url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(CHECKIN_SIGNATURE_HEADER, signature);
    if let Some(tenant_id) = &checkin.tenant_id {
        request = request.header(CHECKIN_TENANT_HEADER, tenant_id.as_str());
    }
    let response = request.body(body).send().await?;
    let status = response.status();
    if status.is_success() {
        return Ok(status.as_u16());
    }
    Err(LicenceCheckinError::Status { status })
}
