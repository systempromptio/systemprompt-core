//! The route pins of the core declaration table are complete by
//! construction: every route a pinned row's router declares, read from the
//! router source, matches one of the row's pins in method and path, every pin
//! covers such a route, and no route of an unpinned router at the same base
//! path lies under a sibling's pin. A route added outside its row's pins
//! would answer 404 (`pinned` / `excluding`), so this test is the one that
//! names it.
//!
//! The router function that builds each row's router is listed once below;
//! a row that gains pins without an entry fails the test. A pinned router's
//! paths are read as string literals; an unpinned router's path may also be a
//! `format!` whose literal head is judged. Any other path expression fails
//! the test naming its site.

use systemprompt_api::services::middleware::{CoreDeclaration, PinnedMethod, path_under};

use super::inventory::{RouteKey, router_inventory};

const ROW_ROUTERS: [(CoreDeclaration, &str); 3] = [
    (
        CoreDeclaration::ContentPublic,
        "content/http/mod.rs::public_router",
    ),
    (
        CoreDeclaration::ContentAuthenticated,
        "content/http/mod.rs::authenticated_router",
    ),
    (
        CoreDeclaration::ContentRedirect,
        "content/http/mod.rs::redirect_router",
    ),
];

fn router_site(row: CoreDeclaration) -> &'static str {
    ROW_ROUTERS
        .iter()
        .find(|(listed, _)| *listed == row)
        .map(|(_, site)| *site)
        .unwrap_or_else(|| panic!("{row:?} pins routes but names no router function here"))
}

fn method_name(method: PinnedMethod) -> &'static str {
    match method {
        PinnedMethod::Get => "GET",
        PinnedMethod::Post => "POST",
    }
}

fn literal(key: &RouteKey) -> &str {
    key.path
        .strip_prefix('"')
        .and_then(|path| path.strip_suffix('"'))
        .unwrap_or_else(|| panic!("{key}: a pinned row's route path must be a string literal"))
}

fn literal_head(key: &RouteKey) -> String {
    if key.path.starts_with('"') {
        return literal(key).to_owned();
    }
    let template = key
        .path
        .strip_prefix("&format!(\"")
        .and_then(|rest| rest.split('"').next())
        .unwrap_or_else(|| panic!("{key}: unreadable route path expression"));
    let head = template.split("{}").next().unwrap_or(template);
    head.replace("{{", "{").replace("}}", "}")
}

fn routes_of(inventory: &[RouteKey], site: &str) -> Vec<RouteKey> {
    let routes: Vec<RouteKey> = inventory
        .iter()
        .filter(|key| key.site == site)
        .cloned()
        .collect();
    assert!(!routes.is_empty(), "no routes read for {site}");
    routes
}

fn siblings(row: CoreDeclaration) -> impl Iterator<Item = CoreDeclaration> {
    CoreDeclaration::ALL.iter().copied().filter(move |other| {
        *other != row
            && other.declarer() == row.declarer()
            && other.base_path() == row.base_path()
            && other.group() == row.group()
    })
}

#[test]
fn every_route_of_a_pinned_row_matches_one_of_its_pins() {
    let inventory: Vec<RouteKey> = router_inventory().into_iter().collect();
    let pinned_rows: Vec<CoreDeclaration> = CoreDeclaration::ALL
        .iter()
        .copied()
        .filter(|row| !row.route_pins().is_empty())
        .collect();
    assert!(pinned_rows.len() >= 2, "pinned rows: {pinned_rows:?}");
    for row in pinned_rows {
        let routes = routes_of(&inventory, router_site(row));
        for key in &routes {
            let path = literal(key);
            assert!(
                row.route_pins().iter().any(|pin| {
                    method_name(pin.method) == key.method && path_under(path, pin.prefix)
                }),
                "{key} lies outside the pins of {row:?} and would answer 404"
            );
        }
        for pin in row.route_pins() {
            assert!(
                routes.iter().any(|key| {
                    method_name(pin.method) == key.method && path_under(literal(key), pin.prefix)
                }),
                "{row:?} pin {pin:?} covers no route"
            );
        }
    }
}

#[test]
fn no_unpinned_router_declares_a_route_under_a_siblings_pin() {
    let inventory: Vec<RouteKey> = router_inventory().into_iter().collect();
    for row in CoreDeclaration::ALL
        .iter()
        .copied()
        .filter(|row| row.route_pins().is_empty())
    {
        let claimed: Vec<&str> = siblings(row)
            .flat_map(|sibling| sibling.route_pins().iter().map(|pin| pin.prefix))
            .collect();
        if claimed.is_empty() {
            continue;
        }
        for key in routes_of(&inventory, router_site(row)) {
            let head = literal_head(&key);
            assert!(
                !claimed.iter().any(|prefix| path_under(&head, prefix)),
                "{key} lies under a sibling row's pin and would answer 404 from {row:?}"
            );
        }
    }
}
