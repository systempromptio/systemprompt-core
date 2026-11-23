//! Route tests for `/api/v1/brain` and `/api/v1/admin/brain`, served by
//! `systemprompt-brain-entry`: the route sets over a brain whose handle is
//! unbound (404 `brain_disabled`), the brain handle the slot build returns,
//! request validation and the error envelope each brain failure maps to.

#[cfg(test)]
mod error_mapping;
#[cfg(test)]
mod handle;
#[cfg(test)]
mod routes;
