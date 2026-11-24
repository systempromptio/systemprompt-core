//! Forwarding to the replica that owns an MCP or agent process.
//!
//! A node that supervises no processes (`api`) has no local backend port to
//! dial, and a `worker` has none for a process another worker's lease covers.
//! Either looks up the live owner in the service registry and replays the
//! request, unchanged and with its own credentials, against the owner's
//! advertised address, where the owner's proxy route authenticates and serves
//! it as if it had arrived directly. No live owner is a 503 on an `api` node,
//! never a guess. A found owner is remembered for [`OWNER_CACHE_TTL`] so a
//! burst of requests costs one registry read; a miss is never remembered, so
//! a name nothing runs cannot fill the cache.
//!
//! A process has no owner for a moment while it moves between workers: the
//! old one has stopped it and released its lease, and the new one is still
//! starting it. An `api` node holds a request for a supervised process
//! through that moment, re-reading the registry every
//! [`OWNER_HANDOVER_POLL`] for at most [`OWNER_HANDOVER_WAIT`], and forwards
//! it once the new owner appears. Requests for one service share the deadline
//! and the reads ([`HandoverWaits`]); once the deadline has passed with no
//! owner, later requests are answered 503 at once until an owner is found
//! again, so a process that cannot start costs one wait, not one per request.
//! A name the configuration does not supervise is never waited for. A forwarded
//! request carries [`FORWARDED_TO_OWNER`] holding a short-lived marker signed
//! with the deployment key for that one service, and a worker receiving a
//! marker it verifies serves the request locally rather than forwarding again,
//! so two workers whose cached owners disagree during a handover cannot bounce
//! a request between them. A marker a client supplies does not verify and is
//! ignored.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::{OriginalUri, Request};
use axum::http::{HeaderMap, HeaderName, HeaderValue};
use axum::response::Response;
use systemprompt_database::ProcessOwner;
use systemprompt_identifiers::ServiceName;
use systemprompt_runtime::AppContext;
use systemprompt_security::jwt::{mint_owner_hop, verify_owner_hop};

use super::ProxyService;
use crate::services::proxy::backend::{ProxyError, ResponseHandler};
use crate::services::server::lifecycle::desired_processes;
use systemprompt_models::net::AGENT_REQUEST_BODY_LIMIT_BYTES;

pub fn owner_url(advertise_url: &str, path_and_query: &str) -> String {
    let base = advertise_url.trim_end_matches('/');
    if path_and_query.starts_with('/') {
        format!("{base}{path_and_query}")
    } else {
        format!("{base}/{path_and_query}")
    }
}

pub const FORWARDED_TO_OWNER: HeaderName =
    HeaderName::from_static(systemprompt_identifiers::headers::OWNER_HOP);

pub const OWNER_CACHE_TTL: Duration = Duration::from_secs(2);
pub const OWNER_HANDOVER_WAIT: Duration = Duration::from_secs(10);
pub const OWNER_HANDOVER_POLL: Duration = Duration::from_millis(200);

/// Recently found process owners, by service name.
#[derive(Debug, Default)]
pub struct OwnerCache {
    entries: Mutex<HashMap<ServiceName, (Instant, ProcessOwner)>>,
}

impl OwnerCache {
    pub fn get(&self, name: &ServiceName) -> Option<ProcessOwner> {
        let entries = self.entries.lock().ok()?;
        entries
            .get(name)
            .filter(|(found, _)| found.elapsed() < OWNER_CACHE_TTL)
            .map(|(_, owner)| owner.clone())
    }

    pub fn put(&self, name: &ServiceName, owner: &ProcessOwner) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.retain(|_, (found, _)| found.elapsed() < OWNER_CACHE_TTL);
            entries.insert(name.clone(), (Instant::now(), owner.clone()));
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct HandoverWait {
    deadline: Instant,
    probed: Instant,
}

/// Hand-overs being waited out, by service name.
#[derive(Debug, Default)]
pub struct HandoverWaits {
    entries: Mutex<HashMap<ServiceName, HandoverWait>>,
}

impl HandoverWaits {
    pub fn deadline(&self, name: &ServiceName, now: Instant) -> Instant {
        let Ok(mut entries) = self.entries.lock() else {
            return now;
        };
        entries
            .entry(name.clone())
            .or_insert(HandoverWait {
                deadline: now + OWNER_HANDOVER_WAIT,
                probed: now,
            })
            .deadline
    }

    pub fn probe_due(&self, name: &ServiceName, now: Instant) -> bool {
        let Ok(mut entries) = self.entries.lock() else {
            return true;
        };
        match entries.get_mut(name) {
            Some(wait) if now.duration_since(wait.probed) < OWNER_HANDOVER_POLL => false,
            Some(wait) => {
                wait.probed = now;
                true
            },
            None => true,
        }
    }

    pub fn clear(&self, name: &ServiceName) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.remove(name);
        }
    }
}

async fn supervised(service_name: &ServiceName, ctx: &AppContext) -> bool {
    match desired_processes(ctx).await {
        Ok(wanted) => wanted.iter().any(|key| key.name() == service_name),
        Err(error) => {
            tracing::warn!(service = %service_name, error = %error, "supervised processes could not be listed; not waiting for an owner");
            false
        },
    }
}

impl ProxyService {
    async fn await_handover(
        &self,
        service_name: &ServiceName,
        ctx: &AppContext,
    ) -> Result<Option<ProcessOwner>, ProxyError> {
        if !supervised(service_name, ctx).await {
            return Ok(None);
        }
        let deadline = self.handovers.deadline(service_name, Instant::now());
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Ok(None);
            }
            tokio::time::sleep(remaining.min(OWNER_HANDOVER_POLL)).await;
            if let Some(owner) = self.owners.get(service_name) {
                return Ok(Some(owner));
            }
            if self.handovers.probe_due(service_name, Instant::now())
                && let Some(owner) = self.process_owner(service_name, ctx).await?
            {
                return Ok(Some(owner));
            }
        }
    }

    pub(super) async fn process_owner(
        &self,
        service_name: &ServiceName,
        ctx: &AppContext,
    ) -> Result<Option<ProcessOwner>, ProxyError> {
        if let Some(owner) = self.owners.get(service_name) {
            return Ok(Some(owner));
        }
        let owner = ctx
            .service_repository()
            .find_process_owner(service_name)
            .await
            .map_err(|source| ProxyError::DatabaseError {
                service: service_name.to_string(),
                source,
            })?;
        if let Some(found) = &owner {
            self.owners.put(service_name, found);
            self.handovers.clear(service_name);
        }
        Ok(owner)
    }

    pub(super) async fn forward_to_owner(
        &self,
        service_name: &ServiceName,
        request: Request<Body>,
        ctx: &AppContext,
    ) -> Result<Response<Body>, ProxyError> {
        let owner = match self.process_owner(service_name, ctx).await? {
            Some(owner) => owner,
            None => self
                .await_handover(service_name, ctx)
                .await?
                .ok_or_else(|| ProxyError::NoLiveOwner {
                    service: service_name.to_string(),
                })?,
        };
        self.forward_to(&owner, service_name, request, ctx).await
    }

    pub(super) async fn forward_to(
        &self,
        owner: &ProcessOwner,
        service_name: &ServiceName,
        request: Request<Body>,
        ctx: &AppContext,
    ) -> Result<Response<Body>, ProxyError> {
        let uri = request
            .extensions()
            .get::<OriginalUri>()
            .map_or_else(|| request.uri().clone(), |original| original.0.clone());
        let path_and_query = uri.path_and_query().map_or("/", |pq| pq.as_str());
        let url = owner_url(&owner.advertise_url, path_and_query);
        tracing::debug!(
            service = %service_name,
            owner = %owner.instance_id,
            "Forwarding to the replica that owns the process"
        );

        let marker = mint_owner_hop(ctx.signing_keys(), &ctx.config().jwt_issuer, service_name)
            .map_err(|source| ProxyError::OwnerHopUnavailable {
                service: service_name.to_string(),
                source,
            })?;
        let marker =
            HeaderValue::from_str(&marker).map_err(|source| ProxyError::HopTokenHeader {
                service: service_name.to_string(),
                source,
            })?;
        let mut headers = request.headers().clone();
        headers.insert(FORWARDED_TO_OWNER, marker);
        let response = self
            .send_to_backend(
                request,
                &super::Upstream {
                    url: &url,
                    headers: &headers,
                    service_name,
                    body_limit: AGENT_REQUEST_BODY_LIMIT_BYTES,
                },
            )
            .await?;
        ResponseHandler::build_response(response).map_err(|source| ProxyError::InvalidResponse {
            service: service_name.to_string(),
            source,
        })
    }
}

pub fn forwarded_by_replica(
    headers: &HeaderMap,
    service_name: &ServiceName,
    ctx: &AppContext,
) -> bool {
    let Some(marker) = headers
        .get(FORWARDED_TO_OWNER)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };
    match verify_owner_hop(
        ctx.signing_keys().keyring(),
        &ctx.config().jwt_issuer,
        service_name,
        marker,
    ) {
        Ok(_) => true,
        Err(error) => {
            tracing::debug!(service = %service_name, %error, "Ignoring an owner-hop marker that does not verify");
            false
        },
    }
}
