use anyhow::Result;
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{HeaderValue, Method, Request, StatusCode, header};
use tower::ServiceExt;
use uuid::Uuid;

use super::table::{Family, Payload, Row, Shape, rows_of};
use crate::documents::{BOUNDARY, multipart};

pub(super) const PROBE_BLOB: &[u8] = b"probe\n";

pub(super) const DEFAULT_BODY_LIMIT: usize = 2 * 1024 * 1024;

pub(super) trait Fixture {
    fn owner(&self) -> Router;
    fn stranger(&self) -> Router;
    fn resource(&self) -> &str;
    fn absent(&self) -> String {
        Uuid::new_v4().to_string()
    }
    fn owner_view(&self) -> Request<Body>;
    fn extra_views(&self) -> Vec<Request<Body>> {
        Vec::new()
    }
    fn stranger_id(&self) -> String {
        String::new()
    }
    fn view_router(&self) -> Router {
        self.owner()
    }
    fn body_limit(&self) -> usize {
        DEFAULT_BODY_LIMIT
    }
    fn body_limit_for(&self, _method: &str, _template: &str) -> usize {
        self.body_limit()
    }
    fn oversize_status_for(&self, _template: &str) -> StatusCode {
        StatusCode::PAYLOAD_TOO_LARGE
    }
    fn primary_for(&self, _template: &str) -> (String, String) {
        (self.resource().to_owned(), self.absent())
    }
}

pub(super) fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(Body::empty())
        .expect("request")
}

pub(super) fn with_bearer(router: Router, token: &str) -> Router {
    let value = HeaderValue::from_str(&format!("Bearer {token}")).expect("bearer header");
    router.layer(axum::middleware::map_request(
        move |mut request: Request<Body>| {
            let value = value.clone();
            async move {
                request.headers_mut().insert(header::AUTHORIZATION, value);
                request
            }
        },
    ))
}

pub(super) struct Reply {
    pub(super) status: StatusCode,
    pub(super) body: String,
}

pub(super) async fn send(app: Router, request: Request<Body>) -> Result<Reply> {
    let response = app.oneshot(request).await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 32 * 1024 * 1024).await?;
    Ok(Reply {
        status,
        body: String::from_utf8_lossy(&bytes).into_owned(),
    })
}

pub(super) fn fill(template: &str, primary: &str, sibling: &str) -> String {
    let mut out = String::with_capacity(template.len() + primary.len());
    let mut rest = template;
    let mut first = true;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let close = open + rest[open..].find('}').expect("closed param");
        let name = &rest[open + 1..close];
        if first {
            out.push_str(primary);
            first = false;
        } else if name.starts_with('*') {
            out.push_str("probe.txt");
        } else {
            out.push_str(sibling);
        }
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    out
}

fn build(method: &str, uri: &str, payload: Payload, primary: &str, caller: &str) -> Request<Body> {
    let method = Method::from_bytes(method.as_bytes()).expect("method");
    let unsafe_method = !method.is_safe();
    let mut builder = Request::builder().method(method).uri(uri);
    if unsafe_method {
        builder = builder.header(header::IF_MATCH, "\"8f14e45f-ceea-4e7a-9d3f-6a1e2b4c5d60\"");
    }
    match payload {
        Payload::Empty => builder.body(Body::empty()),
        Payload::Json(template) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                template.replace("$ID", primary).replace("$CALLER", caller),
            )),
        Payload::Built(make) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(make(primary))),
        Payload::Raw => builder
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .body(Body::from("probe bytes")),
        Payload::Multipart(template) => {
            let meta: serde_json::Value =
                serde_json::from_str(&template.replace("$ID", primary)).expect("multipart meta");
            builder
                .header(
                    header::CONTENT_TYPE,
                    format!("multipart/form-data; boundary={BOUNDARY}"),
                )
                .body(Body::from(multipart(&meta, PROBE_BLOB)))
        },
    }
    .expect("request")
}

fn oversize(method: &str, uri: &str, payload: Payload, limit: usize) -> Request<Body> {
    let method = Method::from_bytes(method.as_bytes()).expect("method");
    let content_type = match payload {
        Payload::Raw => "application/octet-stream".to_owned(),
        Payload::Multipart(_) => format!("multipart/form-data; boundary={BOUNDARY}"),
        Payload::Json(_) | Payload::Built(_) | Payload::Empty => "application/json".to_owned(),
    };
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, limit + 1)
        .body(Body::from(vec![b' '; limit + 1]))
        .expect("request")
}

fn normalise(body: &str, ids: &[&str]) -> String {
    let replaced = ids
        .iter()
        .fold(body.to_owned(), |acc, id| acc.replace(id, "<id>"));
    match serde_json::from_str::<serde_json::Value>(&replaced) {
        Ok(mut value) => {
            drop_volatile(&mut value);
            value.to_string()
        },
        Err(_) => replaced,
    }
}

fn drop_volatile(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            map.remove("timestamp");
            map.remove("trace_id");
            map.values_mut().for_each(drop_volatile);
        },
        serde_json::Value::Array(items) => items.iter_mut().for_each(drop_volatile),
        _ => {},
    }
}

async fn owner_state(fixture: &impl Fixture) -> Result<Reply> {
    let mut state = send(fixture.view_router(), fixture.owner_view()).await?;
    state.body = normalise(&state.body, &[]);
    for view in fixture.extra_views() {
        let extra = send(fixture.view_router(), view).await?;
        state.body.push('\n');
        state.body.push_str(&normalise(&extra.body, &[]));
    }
    Ok(state)
}

pub(super) async fn sweep(fixture: &impl Fixture, family: Family) -> Result<()> {
    let mut failures = Vec::new();
    let baseline = owner_state(fixture).await?;
    assert!(
        baseline.status.is_success(),
        "{family:?} owner view: {} {}",
        baseline.status,
        baseline.body
    );
    assert!(
        baseline.body.contains(fixture.resource()),
        "{family:?} owner view does not carry the seeded resource: {}",
        baseline.body
    );
    for row in rows_of(family) {
        probe_row(fixture, &row, &baseline.body, &mut failures).await?;
    }
    assert!(
        failures.is_empty(),
        "{family:?} ownership sweep:\n  {}",
        failures.join("\n  ")
    );
    Ok(())
}

async fn probe_row(
    fixture: &impl Fixture,
    row: &Row,
    baseline: &str,
    failures: &mut Vec<String>,
) -> Result<()> {
    let probe = row.probe().expect("owned row");
    let label = format!("{} {}", row.method, row.route_template());
    let sibling = Uuid::new_v4().to_string();
    let (owned_id, absent) = fixture.primary_for(row.route_template());
    let resource = owned_id.as_str();
    let uri = fill(row.route_template(), resource, &sibling);
    let caller = fixture.stranger_id();
    let request = || build(row.method, &uri, probe.payload, resource, &caller);
    match probe.shape {
        Shape::NotFound | Shape::Forbidden => {
            let expected = if probe.shape == Shape::NotFound {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::FORBIDDEN
            };
            let theirs = send(fixture.stranger(), request()).await?;
            if theirs.status != expected {
                failures.push(format!(
                    "{label}: a second user got {} (expected {expected}): {}",
                    theirs.status, theirs.body
                ));
            } else if probe.shape == Shape::NotFound {
                let absent_uri = fill(row.route_template(), &absent, &sibling);
                let missing = send(
                    fixture.stranger(),
                    build(row.method, &absent_uri, probe.payload, &absent, &caller),
                )
                .await?;
                let ids = [resource, absent.as_str()];
                if missing.status != theirs.status
                    || normalise(&missing.body, &ids) != normalise(&theirs.body, &ids)
                {
                    failures.push(format!(
                        "{label}: refusal discloses existence: owned → {} {} / absent → {} {}",
                        theirs.status, theirs.body, missing.status, missing.body
                    ));
                }
            }
        },
        Shape::Listing | Shape::CallerScoped => {
            let theirs = send(fixture.stranger(), request()).await?;
            if theirs.body.contains(fixture.resource()) || theirs.body.contains(resource) {
                failures.push(format!(
                    "{label}: a second user's reply carries the owner's resource: {} {}",
                    theirs.status, theirs.body
                ));
            }
        },
        Shape::Create => {},
    }
    let after = owner_state(fixture).await?;
    if after.body != baseline {
        failures.push(format!(
            "{label}: the owner's resource changed after a second user's request: {} → {}",
            baseline, after.body
        ));
    }
    if !matches!(probe.payload, Payload::Empty) {
        let limit = fixture.body_limit_for(row.method, row.route_template());
        let big = send(
            fixture.owner(),
            oversize(row.method, &uri, probe.payload, limit),
        )
        .await?;
        if big.status != fixture.oversize_status_for(row.route_template()) {
            failures.push(format!(
                "{label}: a body one byte over the {limit} byte limit got {}: {}",
                big.status,
                truncate(&big.body)
            ));
        }
    }
    Ok(())
}

fn truncate(body: &str) -> &str {
    let end = body.char_indices().nth(300).map_or(body.len(), |(i, _)| i);
    &body[..end]
}
