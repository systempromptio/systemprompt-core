//! Integration tests for the Slack and Teams entry routes: the handlers on a
//! fixture `AppContext` with a real database, and the handler-verified
//! refusals on the real mounted router of a messaging node.

#[cfg(test)]
mod common;

#[cfg(test)]
#[path = "declared_groups.rs"]
mod declared_groups;

#[cfg(test)]
#[path = "dispatch_seal.rs"]
mod dispatch_seal;

#[cfg(test)]
#[path = "routes_slack.rs"]
mod routes_slack;

#[cfg(test)]
#[path = "routes_slack_edges.rs"]
mod routes_slack_edges;

#[cfg(test)]
#[path = "routes_slack_more.rs"]
mod routes_slack_more;

#[cfg(test)]
#[path = "routes_slack_payload_edges.rs"]
mod routes_slack_payload_edges;

#[cfg(test)]
#[path = "routes_teams.rs"]
mod routes_teams;

#[cfg(test)]
#[path = "secret_refs.rs"]
mod secret_refs;
