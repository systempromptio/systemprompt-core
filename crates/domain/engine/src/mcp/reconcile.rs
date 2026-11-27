//! Works out the smallest set of changes that turns the running servers into a
//! new configuration.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use crate::manager::ServerConfig;

pub(crate) struct Existing<'a> {
    pub(crate) config: &'a ServerConfig,
    pub(crate) live: bool,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Plan {
    pub(crate) remove: Vec<String>,
    pub(crate) start: Vec<String>,
    pub(crate) disable: Vec<String>,
    pub(crate) swap_headers: Vec<String>,
    pub(crate) update: Vec<String>,
}

pub(crate) fn plan(
    current: &BTreeMap<&str, Existing<'_>>,
    next: &BTreeMap<String, ServerConfig>,
) -> Plan {
    let mut plan = Plan::default();
    for name in current.keys() {
        if !next.contains_key(*name) {
            plan.remove.push((*name).to_owned());
        }
    }
    for (name, wanted) in next {
        match current.get(name.as_str()) {
            Some(existing) => plan_existing(&mut plan, name, existing, wanted),
            None if wanted.enabled => plan.start.push(name.clone()),
            None => plan.disable.push(name.clone()),
        }
    }
    plan
}

fn plan_existing(plan: &mut Plan, name: &str, existing: &Existing<'_>, wanted: &ServerConfig) {
    let had = existing.config;
    let name = name.to_owned();
    match (had.enabled, wanted.enabled) {
        (true, false) => plan.disable.push(name),
        (false, false) => {
            if !had.same_as(wanted) {
                plan.update.push(name);
            }
        },
        (false, true) => plan.start.push(name),
        (true, true) => {
            if !had.spec.same_endpoint(&wanted.spec) {
                plan.start.push(name);
            } else if !had.spec.same_headers(&wanted.spec) {
                if existing.live {
                    plan.swap_headers.push(name);
                } else {
                    plan.start.push(name);
                }
            } else if had.connect_timeout != wanted.connect_timeout {
                plan.update.push(name);
            }
        },
    }
}
