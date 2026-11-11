//! Route ownership sweep.
//!
//! The route inventory is read from the `systemprompt-api` router source:
//! every `.route(path, method_router)` call, one key per method. Each key is
//! classified in [`table`] as either user-owned, with a probe, or not
//! user-owned, with a reason. A key in neither place fails the sweep, so a new
//! route cannot land without a decision.
//!
//! A user-owned probe seeds a resource as user A, then asks for it as user B
//! and asserts the refusal the route's contract names. For a `not_found`
//! contract the refusal must equal the reply for an id that does not exist,
//! so the route does not disclose the resource. The owner's view of the
//! resource must be unchanged after every probe. A listing probe asserts B's
//! listing does not carry A's resource, and every probe that takes a body is
//! also sent one byte over the router's body limit and must answer 413.

mod agent;
mod brain;
mod catalog;
mod documents;
mod drive;
mod engine;
mod inventory;
mod issues;
mod notifications;
mod oauth_clients;
mod provider_credentials;
mod runs;
mod table;
mod table_rows;
mod uploads;
