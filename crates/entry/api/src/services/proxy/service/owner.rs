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
//! a name nothing runs cannot fill the cache. A forwarded request carries
//! [`FORWARDED_TO_OWNER`], and a worker receiving one serves it locally
//! rather than forwarding again, so two workers whose cached owners disagree
//! during a handover cannot bounce a request between them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::{OriginalUri, Request};
use axum::http::{HeaderName, HeaderValue};
use axum::response::Response;
use systemprompt_database::ProcessOwner;
use systemprompt_identifiers::ServiceName;
use systemprompt_runtime::AppContext;

use super::ProxyService;
use crate::services::proxy::backend::{ProxyError, ResponseHandler};

pub fn owner_url(advertise_url: &str, path_and_query: &str) -> String {
    let base = advertise_url.trim_end_matches('/');
    if path_and_query.starts_with('/') {
        format!("{base}{path_and_query}")
    } else {
        format!("{base}/{path_and_query}")
    }
}

pub const FORWARDED_TO_OWNER: HeaderName = HeaderName::from_static("x-systemprompt-owner-hop");

pub const OWNER_CACHE_TTL: Duration = Duration::from_secs(2);

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

impl ProxyService {
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
        }
        Ok(owner)
    }

    pub(super) async fn forward_to_owner(
        &self,
        service_name: &ServiceName,
        request: Request<Body>,
        ctx: &AppContext,
    ) -> Result<Response<Body>, ProxyError> {
        let owner = self
            .process_owner(service_name, ctx)
            .await?
            .ok_or_else(|| ProxyError::NoLiveOwner {
                service: service_name.to_string(),
            })?;
        self.forward_to(&owner, service_name, request).await
    }

    pub(super) async fn forward_to(
        &self,
        owner: &ProcessOwner,
        service_name: &ServiceName,
        request: Request<Body>,
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

        let mut headers = request.headers().clone();
        headers.insert(FORWARDED_TO_OWNER, HeaderValue::from_static("1"));
        let response = self
            .send_to_backend(request, &url, &headers, service_name)
            .await?;
        ResponseHandler::build_response(response).map_err(|source| ProxyError::InvalidResponse {
            service: service_name.to_string(),
            source,
        })
    }
}
