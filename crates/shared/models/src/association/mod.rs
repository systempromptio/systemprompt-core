//! The two app association documents an instance serves, and the declaration
//! of the apps it vouches for.
//!
//! A domain vouches for a native app by serving a vendor-defined document
//! under `/.well-known/`: [`AppleAppSiteAssociation`] at
//! `apple-app-site-association` and [`AssetLinks`] at `assetlinks.json`. With
//! them an app may use the domain's passkeys and open the domain's links.
//! Both types serialise to exactly the vendor's JSON.
//!
//! [`VouchedApps`] is the configuration shape that declares the apps. Each
//! document is a pure function of it
//! ([`VouchedApps::apple_app_site_association`], [`VouchedApps::asset_links`])
//! and is `None` when no app of that vendor is declared, so an instance with no
//! apps serves neither file.
//!
//! ```
//! use systemprompt_models::association::VouchedApps;
//!
//! let apps: VouchedApps = serde_json::from_str(
//!     r#"{"ios": [{"team_id": "ABCDE12345", "bundle_id": "io.example.app"}]}"#,
//! )?;
//! apps.validate()?;
//! assert!(apps.apple_app_site_association().is_some());
//! assert!(apps.asset_links().is_none());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod android;
mod apple;
mod config;
mod ids;

pub use android::{
    ANDROID_ASSET_RELATIONS, AssetLinkNamespace, AssetLinkRelation, AssetLinkStatement,
    AssetLinkTarget, AssetLinks,
};
pub use apple::{
    AppleAppLinkDetail, AppleAppLinks, AppleAppSiteAssociation, AppleLinkComponent,
    AppleWebCredentials,
};
pub use config::{AndroidAppDecl, AppleAppDecl, VouchedApps, VouchedAppsError};
pub use ids::{
    AppleAppId, AppleAppIdError, AppleTeamId, AssociationValueError, Sha256CertFingerprint,
};
