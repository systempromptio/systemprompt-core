//! Integration tests for the Slack and Teams entry routes: the handlers on a
//! fixture `AppContext` with a real database, and the handler-verified
//! refusals on the real mounted router of a messaging node.

#[cfg(test)]
mod common;

#[cfg(test)]
mod declared_groups;

#[cfg(test)]
mod dispatch_seal;

#[cfg(test)]
mod route_state_seeds;

#[cfg(test)]
mod routes_slack;

#[cfg(test)]
mod routes_slack_edges;

#[cfg(test)]
mod routes_slack_more;

#[cfg(test)]
mod routes_slack_payload_edges;

#[cfg(test)]
mod routes_teams;

#[cfg(test)]
mod secret_refs;
