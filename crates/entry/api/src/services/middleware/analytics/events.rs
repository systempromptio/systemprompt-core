use serde_json::json;
use std::sync::Arc;

use systemprompt_core_logging::{AnalyticsEvent, AnalyticsRepository};
use systemprompt_models::{RequestContext, RouteClassifier};

#[derive(Debug)]
pub struct AnalyticsEventParams {
    pub req_ctx: RequestContext,
    pub endpoint: String,
    pub path: String,
    pub method: String,
    pub uri: http::Uri,
    pub status_code: u16,
    pub response_time_ms: u64,
    pub user_agent: Option<String>,
    pub referer: Option<String>,
}

pub fn spawn_analytics_event_task(
    analytics_repo: Arc<AnalyticsRepository>,
    route_classifier: Arc<RouteClassifier>,
    params: AnalyticsEventParams,
) {
    let sanitized_uri = sanitize_uri(&params.uri);

    tokio::spawn(async move {
        let message = format!(
            "HTTP {} - {} {}",
            params.status_code, params.method, sanitized_uri
        );
        let metadata = json!({
            "status_code": params.status_code,
            "method": params.method,
            "uri": sanitized_uri,
            "endpoint": params.endpoint,
            "trace_id": params.req_ctx.trace_id(),
            "user_agent": params.user_agent,
            "referer": params.referer
        });

        let event_metadata = route_classifier.get_event_metadata(&params.path, &params.method);

        let severity = if params.status_code >= 500 {
            "error"
        } else if params.status_code >= 400 {
            "warning"
        } else {
            "info"
        };

        let event = AnalyticsEvent {
            user_id: params.req_ctx.auth.user_id.clone(),
            session_id: params.req_ctx.request.session_id.clone(),
            context_id: params.req_ctx.execution.context_id.clone(),
            event_type: event_metadata.event_type.to_string(),
            event_category: event_metadata.event_category.to_string(),
            severity: severity.to_string(),
            endpoint: Some(params.endpoint),
            error_code: if params.status_code >= 400 {
                Some(i32::from(params.status_code))
            } else {
                None
            },
            response_time_ms: Some(params.response_time_ms as i32),
            agent_id: None,
            task_id: params.req_ctx.task_id().cloned(),
            message: Some(message.clone()),
            metadata: metadata.clone(),
        };

        if let Err(e) = analytics_repo.log_event(&event).await {
            tracing::error!(error = %e, "Failed to log analytics event");
        }

        if params.status_code >= 500 {
            tracing::error!(module = event_metadata.log_module, message = %message, metadata = ?metadata, "HTTP error");
        }
    });
}

fn sanitize_uri(uri: &http::Uri) -> String {
    let path = uri.path();

    uri.query().map_or_else(
        || path.to_string(),
        |query| {
            let sanitized_params: Vec<String> = query
                .split('&')
                .map(|param| {
                    param.split_once('=').map_or_else(
                        || param.to_string(),
                        |(key, value)| {
                            let key_lower = key.to_lowercase();
                            if is_sensitive_key(&key_lower) {
                                format!("{key}=[REDACTED]")
                            } else {
                                format!("{key}={value}")
                            }
                        },
                    )
                })
                .collect();

            format!("{path}?{}", sanitized_params.join("&"))
        },
    )
}

fn is_sensitive_key(key: &str) -> bool {
    matches!(
        key,
        "token" | "password" | "api_key" | "apikey" | "secret" | "authorization" | "auth"
    )
}
