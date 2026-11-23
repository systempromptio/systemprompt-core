//! `GET|POST /s/{token}`: the anonymous open of a document's public link.
//!
//! The route reads no session and sets no cookie (the session tracker skips
//! `/s/`). `POST` carries the link's password as a form field. A served
//! document is never cached, sends no referrer, asks not to be indexed and
//! carries a sandboxing CSP: an HTML document is served inline under the
//! rendered view's policy, Markdown and text inline as plain text, and any
//! other format as a download. Every refusal but throttling answers the one
//! `404` page, which also offers the password form, so an unknown, revoked,
//! expired or withheld link and a missing or wrong password look the same.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Form, Router};
use serde::Deserialize;
use systemprompt_files::documents::DocumentService;
use systemprompt_files::documents::service::{PublicLinkOpen, PublicLinkRefusal};
use systemprompt_models::documents::DocumentFormat;

use crate::routes::user_content::harden;
use crate::services::middleware::client_addr::ClientIp;

const DOWNLOAD_CSP: HeaderValue = HeaderValue::from_static("sandbox; default-src 'none'");
const PAGE_CSP: HeaderValue = HeaderValue::from_static(
    "default-src 'none'; style-src 'unsafe-inline'; form-action 'self'; base-uri 'none'; \
     frame-ancestors 'none'",
);
const UNAVAILABLE_PAGE: &str = "<!doctype html><html lang=\"en\"><head><meta \
     charset=\"utf-8\"><meta name=\"robots\" content=\"noindex\"><title>Link \
     unavailable</title></head><body style=\"font-family:sans-serif;max-width:32rem;margin:4rem \
     auto\"><h1>This link is unavailable</h1><p>It may have expired or been withdrawn, or it \
     needs a password.</p><form method=\"post\"><label>Password <input type=\"password\" \
     name=\"password\" autocomplete=\"off\" required></label> <button>Open</button></form>\
     </body></html>";

#[derive(Debug, Deserialize)]
pub(crate) struct PasswordForm {
    password: String,
}

pub fn router(service: Arc<DocumentService>) -> Router {
    Router::new()
        .route("/{token}", get(open).post(open_with_password))
        .with_state(service)
}

async fn open(
    State(service): State<Arc<DocumentService>>,
    Path(token): Path<String>,
    ClientIp(ip): ClientIp,
) -> Response {
    respond(service.open_public_link(&token, None, ip).await, &service)
}

async fn open_with_password(
    State(service): State<Arc<DocumentService>>,
    Path(token): Path<String>,
    ClientIp(ip): ClientIp,
    Form(form): Form<PasswordForm>,
) -> Response {
    respond(
        service
            .open_public_link(&token, Some(&form.password), ip)
            .await,
        &service,
    )
}

fn respond(
    outcome: Result<PublicLinkOpen, PublicLinkRefusal>,
    service: &DocumentService,
) -> Response {
    match outcome {
        Ok(open) => served(open, service),
        Err(PublicLinkRefusal::Unavailable) => page(StatusCode::NOT_FOUND, UNAVAILABLE_PAGE),
        Err(PublicLinkRefusal::Throttled) => page(
            StatusCode::TOO_MANY_REQUESTS,
            "<!doctype html><title>Too many attempts</title><p>Too many password attempts. Try \
             again later.</p>",
        ),
        Err(PublicLinkRefusal::Store(error)) => {
            tracing::error!(error = %error, "public link open failed");
            page(
                StatusCode::INTERNAL_SERVER_ERROR,
                "<!doctype html><title>Error</title><p>Something went wrong.</p>",
            )
        },
    }
}

fn served(open: PublicLinkOpen, service: &DocumentService) -> Response {
    let (content_type, disposition, csp) = match open.document.format {
        DocumentFormat::Html => (
            HeaderValue::from_static("text/html; charset=utf-8"),
            "inline",
            service.config().rendered_html.csp(open.agent_content),
        ),
        DocumentFormat::Markdown | DocumentFormat::Text => (
            HeaderValue::from_static("text/plain; charset=utf-8"),
            "inline",
            DOWNLOAD_CSP,
        ),
        _ => (
            HeaderValue::from_str(&open.document.mime_type)
                .unwrap_or(HeaderValue::from_static("application/octet-stream")),
            "attachment",
            DOWNLOAD_CSP,
        ),
    };
    let mut response = Response::new(Body::from_stream(open.read.body));
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, content_type);
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static(disposition),
    );
    private_headers(&mut response);
    harden(&mut response, csp);
    response
}

fn page(status: StatusCode, body: &'static str) -> Response {
    let mut response = (status, body).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    private_headers(&mut response);
    harden(&mut response, PAGE_CSP);
    response
}

fn private_headers(response: &mut Response) {
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        header::HeaderName::from_static("x-robots-tag"),
        HeaderValue::from_static("noindex, nofollow"),
    );
}
