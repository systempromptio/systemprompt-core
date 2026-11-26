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

use std::future::{Future, ready};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{ConnectInfo, FromRequestParts, Path, State};
use axum::http::request::Parts;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Form, Router};
use serde::Deserialize;
use systemprompt_extension::router::ClientIpResolver;
use systemprompt_files::documents::DocumentService;
use systemprompt_files::documents::service::{PublicLinkOpen, PublicLinkRefusal};
use systemprompt_models::documents::DocumentFormat;

use crate::http::user_content::harden;

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

/// Router-scoped state of the public-link open: the document service and
/// the server's client-address resolver the password throttle keys on.
#[derive(Clone)]
pub struct PublicLinkState {
    service: Arc<DocumentService>,
    client_ip: ClientIpResolver,
}

impl std::fmt::Debug for PublicLinkState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PublicLinkState").finish_non_exhaustive()
    }
}

struct CallerIp(Option<IpAddr>);

impl FromRequestParts<PublicLinkState> for CallerIp {
    type Rejection = std::convert::Infallible;

    fn from_request_parts(
        parts: &mut Parts,
        state: &PublicLinkState,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        ready(Ok(Self((state.client_ip)(
            &parts.headers,
            parts.extensions.get::<ConnectInfo<SocketAddr>>(),
        ))))
    }
}

pub fn router(service: Arc<DocumentService>, client_ip: ClientIpResolver) -> Router {
    Router::new()
        .route("/{token}", get(open).post(open_with_password))
        .with_state(PublicLinkState { service, client_ip })
}

async fn open(
    State(PublicLinkState { service, .. }): State<PublicLinkState>,
    Path(token): Path<String>,
    CallerIp(ip): CallerIp,
) -> Response {
    respond(service.open_public_link(&token, None, ip).await, &service)
}

async fn open_with_password(
    State(PublicLinkState { service, .. }): State<PublicLinkState>,
    Path(token): Path<String>,
    CallerIp(ip): CallerIp,
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
