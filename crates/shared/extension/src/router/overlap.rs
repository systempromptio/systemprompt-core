//! Where admitted routes land and when two of them claim one request.
//!
//! A full path joins a base and a declared path. Two handler defs overlap on
//! the same method and full path; a service def ([`RouteMethod::Any`])
//! overlaps every def whose full path its prefix covers, and another service
//! whose prefix covers or is covered by its own.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::shape::{BaseRef, RouteDef, RouteMethod};

/// Two admitted defs that claim one request: the same method and full path,
/// or a service prefix covering another def's full path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteOverlap {
    pub method: RouteMethod,
    pub path: String,
}

#[must_use]
pub fn full_path(base: BaseRef, path: &str) -> String {
    let base = match base {
        BaseRef::Path(base) => base.path(),
        BaseRef::Gateway(gateway) => gateway.path(),
    };
    match (base, path) {
        ("/", path) => path.to_owned(),
        (base, "/") => base.to_owned(),
        (base, path) => format!("{base}{path}"),
    }
}

#[must_use]
pub fn path_covers(prefix: &str, path: &str) -> bool {
    if prefix == "/" {
        return path == "/";
    }
    path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}

#[must_use]
pub fn first_overlap<'a>(
    defs: impl IntoIterator<Item = (BaseRef, &'a RouteDef)>,
) -> Option<RouteOverlap> {
    let mut seen: Vec<(&'a RouteMethod, String)> = Vec::new();
    for (base, def) in defs {
        let path = full_path(base, def.path.as_str());
        let clash = seen.iter().any(|(method, earlier)| match (*method, &def.method) {
            (RouteMethod::One(_), RouteMethod::One(_)) => {
                **method == def.method && *earlier == path
            },
            (RouteMethod::Any, RouteMethod::One(_)) => path_covers(earlier, &path),
            (RouteMethod::One(_), RouteMethod::Any) => path_covers(&path, earlier),
            (RouteMethod::Any, RouteMethod::Any) => {
                path_covers(earlier, &path) || path_covers(&path, earlier)
            },
        });
        if clash {
            return Some(RouteOverlap {
                method: def.method.clone(),
                path,
            });
        }
        seen.push((&def.method, path));
    }
    None
}
