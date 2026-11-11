use std::collections::{BTreeMap, BTreeSet};

use super::inventory::{RouteKey, router_inventory};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Family {
    Documents,
    Folders,
    Drive,
    Uploads,
    Issues,
    AgentRuns,
    Contexts,
    Tasks,
    Artifacts,
    McpExecutions,
    Webhooks,
    Notifications,
    Subscriptions,
    BridgeNotifications,
    Brain,
    OAuthClients,
    ProviderCredentials,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Shape {
    NotFound,
    Forbidden,
    Listing,
    Create,
    CallerScoped,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum Payload {
    Empty,
    Json(&'static str),
    Built(fn(&str) -> String),
    Multipart(&'static str),
    Raw,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Probe {
    pub(super) family: Family,
    pub(super) shape: Shape,
    pub(super) uri: Option<&'static str>,
    pub(super) payload: Payload,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum Class {
    Owned(Probe),
    NotUserOwned(&'static str),
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Row {
    pub(super) site: &'static str,
    pub(super) method: &'static str,
    pub(super) path: &'static str,
    pub(super) class: Class,
}

impl Row {
    pub(super) fn key(&self) -> RouteKey {
        RouteKey {
            site: self.site.to_owned(),
            method: self.method.to_owned(),
            path: self.path.to_owned(),
        }
    }

    pub(super) fn probe(&self) -> Option<Probe> {
        match self.class {
            Class::Owned(probe) => Some(probe),
            Class::NotUserOwned(_) => None,
        }
    }

    pub(super) fn route_template(&self) -> &'static str {
        self.probe()
            .and_then(|p| p.uri)
            .unwrap_or_else(|| self.path.trim_matches('"'))
    }
}

pub(super) const fn owned(
    site: &'static str,
    method: &'static str,
    path: &'static str,
    family: Family,
    shape: Shape,
    payload: Payload,
) -> Row {
    Row {
        site,
        method,
        path,
        class: Class::Owned(Probe {
            family,
            shape,
            uri: None,
            payload,
        }),
    }
}

pub(super) const fn owned_at(
    site: &'static str,
    method: &'static str,
    path: &'static str,
    uri: &'static str,
    probe: (Family, Shape, Payload),
) -> Row {
    Row {
        site,
        method,
        path,
        class: Class::Owned(Probe {
            family: probe.0,
            shape: probe.1,
            uri: Some(uri),
            payload: probe.2,
        }),
    }
}

pub(super) const fn free(
    site: &'static str,
    method: &'static str,
    path: &'static str,
    reason: &'static str,
) -> Row {
    Row {
        site,
        method,
        path,
        class: Class::NotUserOwned(reason),
    }
}

pub(super) fn all_rows() -> Vec<Row> {
    super::table_rows::OWNED
        .iter()
        .chain(super::catalog::NOT_USER_OWNED.iter())
        .copied()
        .collect()
}

pub(super) fn rows_of(family: Family) -> Vec<Row> {
    let rows: Vec<Row> = all_rows()
        .into_iter()
        .filter(|row| row.probe().is_some_and(|p| p.family == family))
        .collect();
    assert!(!rows.is_empty(), "no sweep rows for {family:?}");
    rows
}

#[test]
fn every_router_route_is_classified_and_every_row_is_a_live_route() {
    let inventory = router_inventory();
    let mut listed: BTreeMap<RouteKey, usize> = BTreeMap::new();
    for row in all_rows() {
        if let Class::NotUserOwned(reason) = row.class {
            assert!(reason.len() > 20, "{} needs a real reason", row.key());
        }
        *listed.entry(row.key()).or_default() += 1;
    }
    let duplicated: Vec<String> = listed
        .iter()
        .filter(|(_, n)| **n > 1)
        .map(|(k, _)| k.to_string())
        .collect();
    let listed: BTreeSet<RouteKey> = listed.into_keys().collect();
    let unlisted: Vec<String> = inventory
        .difference(&listed)
        .map(ToString::to_string)
        .collect();
    let stale: Vec<String> = listed
        .difference(&inventory)
        .map(ToString::to_string)
        .collect();
    assert!(
        unlisted.is_empty() && stale.is_empty() && duplicated.is_empty(),
        "ownership sweep table out of step with the router.\n\
         Routes in the router with no sweep row (add a probe, or a not-user-owned reason):\n  {}\n\
         Rows naming no router route:\n  {}\n\
         Rows listed twice:\n  {}",
        unlisted.join("\n  "),
        stale.join("\n  "),
        duplicated.join("\n  "),
    );
}

#[test]
fn the_inventory_sees_methods_bound_through_a_let_and_every_method_of_a_chain() {
    let inventory = router_inventory();
    let has = |site: &str, method: &str, path: &str| {
        inventory.contains(&RouteKey {
            site: site.to_owned(),
            method: method.to_owned(),
            path: path.to_owned(),
        })
    };
    for method in ["GET", "PUT", "DELETE"] {
        assert!(
            has("routes/agent/contexts/mod.rs::router", method, "\"/{id}\""),
            "{method} /contexts/{{id}}"
        );
    }
    for method in ["GET", "HEAD", "PATCH", "DELETE"] {
        assert!(has("routes/documents/mod.rs::router", method, "\"/{id}\""));
    }
    assert!(has(
        "routes/documents/drive.rs::router",
        "POST",
        "ApiPaths::gateway_relative(ApiPaths::BRIDGE_DRIVE_COMMIT)"
    ));
    assert!(inventory.len() > 250, "{} routes", inventory.len());
}
