//! Tests for `systemprompt-marketplace-entry`: the marketplace route set
//! materialized over a fixture context's kernel state — the catalogue
//! renders, the confined manifest read, the plugin-file misses and the
//! services-watch reads.

#[cfg(test)]
mod common;
#[cfg(test)]
mod marketplace_yaml;
#[cfg(test)]
mod routes_marketplace;
#[cfg(test)]
mod routes_marketplace_services_watch;
#[cfg(test)]
mod routes_marketplace_success;
