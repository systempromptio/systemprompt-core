//! Where admitted routes land and when two of them claim one request.
//!
//! A full path joins a base and a declared path; a gateway route set's defs
//! are gateway-relative, so its full path joins the gateway base. Two handler
//! defs overlap on the same method and full path; a service def
//! ([`RouteMethod::Any`]) overlaps every def whose full path its prefix covers,
//! and another service whose prefix covers or is covered by its own.
//!
//! The kernel's own routes are claimed by [`RESERVED_PATHS`]: every route
//! the kernel or a core member mounts lies under one entry, owned by the core
//! id that mounts it. A def *reaches* an entry when any request its pattern
//! matches falls under that entry — a parameter segment matches any segment,
//! a catch-all every remaining path, and a service its prefix and everything
//! below it. A def that reaches an entry its holder does not own is refused
//! ([`refused_reservation`]), so an install literal cannot outrank a kernel
//! parameter route (`/api/v1/mcp/{service_name}/{*path}`), and an install
//! parameter, catch-all or service cannot answer an unclaimed path inside a
//! kernel namespace. A holder routes inside a nested entry it owns
//! (`/api/v1/admin/notifications`) without holding the enclosing one. The
//! owner of an enclosing prefix keeps the rest of it: its parameter segment at
//! the place of another owner's nested literal (`/api/v1/agents/{service_name}`
//! beside `/api/v1/agents/registry`) is not refused, because the literal route
//! takes precedence and the nested owner's row holds it.
//!
//! A gateway route def is confined to the gateway path its set claims: its
//! gateway-relative path lies within the claim, and no built-in gateway
//! prefix covers it ([`refused_gateway_def`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::base_ref::{BaseRef, GatewayPath};
use super::shape::{RouteDef, RouteMethod};
use systemprompt_models::modules::ApiPaths;

use crate::reserved::{
    BUILTIN_GATEWAY_PREFIXES, CoreId, RESERVED_PATHS, ReservedPath, ReservedReach,
};

/// Two admitted defs that claim one request: the same method and full path,
/// or a service prefix covering another def's full path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteOverlap {
    pub method: RouteMethod,
    pub path: String,
}

#[must_use]
pub fn full_path(base: BaseRef, path: &str) -> String {
    match (base.path(), path) {
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
        let clash = seen
            .iter()
            .any(|(method, earlier)| match (*method, &def.method) {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Segment<'a> {
    Literal(&'a str),
    Param,
    CatchAll,
}

fn segments(path: &str) -> Vec<Segment<'_>> {
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            if segment.contains('*') {
                Segment::CatchAll
            } else if segment.contains('{') {
                Segment::Param
            } else {
                Segment::Literal(segment)
            }
        })
        .collect()
}

fn held_segments(entry: &ReservedPath) -> Vec<&'static str> {
    entry
        .path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn literal_matches(entry: &ReservedPath, last: bool, segment: &str, held: &str) -> bool {
    if last && entry.reach == ReservedReach::Prefix {
        segment.starts_with(held)
    } else {
        segment == held
    }
}

fn reaches(pattern: &[Segment<'_>], entry: &ReservedPath) -> bool {
    let held = held_segments(entry);
    for (index, want) in held.iter().enumerate() {
        match pattern.get(index) {
            None => return false,
            Some(Segment::CatchAll) => return true,
            Some(Segment::Param) => {},
            Some(Segment::Literal(segment)) => {
                if !literal_matches(entry, index + 1 == held.len(), segment, want) {
                    return false;
                }
            },
        }
    }
    entry.reach != ReservedReach::Exact || pattern.len() == held.len()
}

fn contained(pattern: &[Segment<'_>], entry: &ReservedPath) -> bool {
    let held = held_segments(entry);
    let leading = held.iter().enumerate().all(|(index, want)| {
        matches!(pattern.get(index), Some(Segment::Literal(segment))
            if literal_matches(entry, index + 1 == held.len(), segment, want))
    });
    leading && (entry.reach != ReservedReach::Exact || pattern.len() == held.len())
}

fn encloses(outer: &ReservedPath, inner: &ReservedPath) -> bool {
    outer != inner && reaches(&segments(inner.path), outer)
}

fn patterns(path: &str, service: bool) -> Vec<Vec<Segment<'_>>> {
    let exact = segments(path);
    if !service {
        return vec![exact];
    }
    let mut below = exact.clone();
    below.push(Segment::CatchAll);
    vec![exact, below]
}

fn nested_param(pattern: &[Segment<'_>], entry: &ReservedPath, own: &[&ReservedPath]) -> bool {
    let held = held_segments(entry);
    let last_is_param = held
        .len()
        .checked_sub(1)
        .is_some_and(|index| matches!(pattern.get(index), Some(Segment::Param)));
    last_is_param
        && own.iter().any(|outer| {
            outer.reach == ReservedReach::Prefix
                && crate::reserved::nested_under_another_owner(entry, std::slice::from_ref(outer))
        })
}

fn first_unheld(
    patterns: &[Vec<Segment<'_>>],
    holder: Option<CoreId>,
) -> Option<&'static ReservedPath> {
    patterns.iter().find_map(|pattern| {
        let own: Vec<&ReservedPath> = RESERVED_PATHS
            .iter()
            .filter(|entry| Some(entry.owner) == holder && contained(pattern, entry))
            .collect();
        RESERVED_PATHS.iter().find(|entry| {
            Some(entry.owner) != holder
                && reaches(pattern, entry)
                && !own.iter().any(|held| encloses(entry, held))
                && !nested_param(pattern, entry, &own)
        })
    })
}

#[must_use]
pub fn refused_reservation(
    base: BaseRef,
    def: &RouteDef,
    holder: Option<CoreId>,
) -> Option<&'static ReservedPath> {
    let path = full_path(base, def.path.as_str());
    first_unheld(&patterns(&path, def.is_service()), holder)
}

#[must_use]
pub fn reserved_below(prefix: &str) -> Option<&'static ReservedPath> {
    first_unheld(&patterns(prefix, true), None)
}

#[must_use]
pub(super) fn reserved_holding(path: &str) -> Option<&'static ReservedPath> {
    let pattern = segments(path);
    RESERVED_PATHS
        .iter()
        .filter(|entry| entry.reach != ReservedReach::Exact && contained(&pattern, entry))
        .max_by_key(|entry| entry.path.len())
}

/// Why a gateway route def is refused at the gateway path its set claims.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayDefRefusal {
    UnderBuiltin { path: String, builtin: &'static str },
    OutsideClaim { path: String, claimed: &'static str },
}

#[must_use]
pub fn refused_gateway_def(claimed: GatewayPath, def: &RouteDef) -> Option<GatewayDefRefusal> {
    let path = def.path.as_str();
    if let Some(builtin) = BUILTIN_GATEWAY_PREFIXES
        .iter()
        .copied()
        .find(|builtin| path_covers(builtin, path))
    {
        return Some(GatewayDefRefusal::UnderBuiltin {
            path: path.to_owned(),
            builtin,
        });
    }
    let claim = ApiPaths::gateway_relative(claimed.path());
    (!path_covers(claim, path)).then(|| GatewayDefRefusal::OutsideClaim {
        path: path.to_owned(),
        claimed: claim,
    })
}
