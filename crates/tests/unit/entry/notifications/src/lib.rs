//! Route tests for `/api/v1/notifications`, served by
//! `systemprompt-notifications-entry`: the user gate, request validation and
//! the error envelope each notification failure maps to, and the calling
//! device's push token with the admin device list.

#[cfg(test)]
mod device_routes;
#[cfg(test)]
mod routes;
#[cfg(test)]
mod rules_routes;
#[cfg(test)]
mod subscription_routes;
#[cfg(test)]
mod support;
