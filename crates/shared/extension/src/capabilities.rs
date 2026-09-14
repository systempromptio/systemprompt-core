//! `Has*` capability traits an extension context implements to advertise
//! which host subsystems it exposes.
//!
//! The host application implements each capability independently on its
//! context type, so an extension that only needs the route classifier is not
//! coupled to analytics or user services.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub trait HasAnalytics: Send + Sync {
    type Analytics: Send + Sync;

    fn analytics(&self) -> &Self::Analytics;
}

pub trait HasFingerprint: Send + Sync {
    type Fingerprint: Send + Sync;

    fn fingerprint(&self) -> Option<&Self::Fingerprint>;
}

pub trait HasUserService: Send + Sync {
    type UserService: Send + Sync;

    fn user_service(&self) -> Option<&Self::UserService>;
}

pub trait HasRouteClassifier: Send + Sync {
    type RouteClassifier: Send + Sync;

    fn route_classifier(&self) -> &Self::RouteClassifier;
}
