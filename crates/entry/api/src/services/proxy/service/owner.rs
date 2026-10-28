//! Forwarding to the replica that owns an MCP or agent process.
//!
//! A node that supervises no processes (`api`) has no local backend port to
//! dial. It looks up the live owner in the service registry and replays the
//! request, unchanged and with its own credentials, against the owner's
//! advertised address, where the owner's proxy route authenticates and serves
//! it as if it had arrived directly. No live owner is a 503, never a guess.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::Body;
use axum::extract::{OriginalUri, Request};
use axum::response::Response;
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

impl ProxyService {
    pub(super) async fn forward_to_owner(
        &self,
        service_name: &ServiceName,
        request: Request<Body>,
        ctx: &AppContext,
    ) -> Result<Response<Body>, ProxyError> {
        let owner = ctx
            .service_repository()
            .find_process_owner(service_name)
            .await
            .map_err(|source| ProxyError::DatabaseError {
                service: service_name.to_string(),
                source,
            })?
            .ok_or_else(|| ProxyError::NoLiveOwner {
                service: service_name.to_string(),
            })?;

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

        let headers = request.headers().clone();
        let response = self
            .send_to_backend(request, &url, &headers, service_name)
            .await?;
        ResponseHandler::build_response(response).map_err(|source| ProxyError::InvalidResponse {
            service: service_name.to_string(),
            source,
        })
    }
}
