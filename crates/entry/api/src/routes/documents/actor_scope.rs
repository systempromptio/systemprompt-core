//! The middleware that runs every document and folder route as the actor
//! the request's credential proves (see
//! [`systemprompt_runtime::documents::actor`]): a first-party browser
//! session is the person, any bearer token an agent. A request that reaches
//! the routes without a context runs unscoped, which the document service
//! treats as an unnamed, unattended agent.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use systemprompt_files::documents::agents::act_as;
use systemprompt_models::RequestContext;
use systemprompt_runtime::documents::actor::request_actor;

pub(super) async fn scope(request: Request, next: Next) -> Response {
    let actor = request
        .extensions()
        .get::<RequestContext>()
        .map(request_actor);
    match actor {
        Some(actor) => act_as(actor, next.run(request)).await,
        None => next.run(request).await,
    }
}
