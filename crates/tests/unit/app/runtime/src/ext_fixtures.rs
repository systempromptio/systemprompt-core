//! Fixture members that make the extension arms of the startup validator
//! reachable, admitted into a [`BuiltState`] by [`fixture_admitted`]:
//! - `covextok`: declares a config section that always validates
//! - `covextbad`: declares a config section that always rejects
//! - `covassets_missing`: declares a required asset that never exists
//! - `covassets_ok`: declares a required asset the boot fixture creates

use serde::Deserialize;
use systemprompt_extension::admitted::{AdmittedMember, AdmittedParts};
use systemprompt_extension::contribute::ContributionSink;
use systemprompt_extension::{
    Admitted, AssetDefinition, ContributionError, Contributions, Extension, ExtensionConfig,
    ExtensionConfigError, ExtensionMetadata, StandingTag,
};
use systemprompt_identifiers::ExtensionId;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CovOkConfig {}

impl ExtensionConfig for CovOkConfig {
    const PREFIX: &'static str = "covextok";

    fn validate(&self) -> Result<(), ExtensionConfigError> {
        Ok(())
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CovBadConfig {}

impl ExtensionConfig for CovBadConfig {
    const PREFIX: &'static str = "covextbad";

    fn validate(&self) -> Result<(), ExtensionConfigError> {
        Err(ExtensionConfigError::InvalidValue {
            key: "covextbad.mode".to_owned(),
            message: "fixture always rejects".to_owned(),
        })
    }
}

struct Fixture {
    id: &'static str,
    contribute: fn(&mut Contributions<'_>),
}

impl Extension for Fixture {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: self.id,
            version: "0.0.1",
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        (self.contribute)(c);
        Ok(())
    }
}

const FIXTURES: [Fixture; 4] = [
    Fixture {
        id: "covextok",
        contribute: |c| {
            c.config::<CovOkConfig>();
        },
    },
    Fixture {
        id: "covextbad",
        contribute: |c| {
            c.config::<CovBadConfig>();
        },
    },
    Fixture {
        id: "covassets_missing",
        contribute: |c| {
            c.asset_with(|paths| {
                vec![AssetDefinition::css(
                    paths.storage_files().join("covassets_missing/absent.css"),
                    "css/absent.css",
                )]
            });
        },
    },
    Fixture {
        id: "covassets_ok",
        contribute: |c| {
            c.asset_with(|paths| {
                vec![AssetDefinition::css(
                    paths.storage_files().join("covassets_ok/present.css"),
                    "css/present.css",
                )]
            });
        },
    },
];

pub fn fixture_admitted() -> Admitted {
    let members = FIXTURES
        .iter()
        .map(|fixture| {
            let owner = ExtensionId::new(fixture.id);
            let mut sink = ContributionSink::default();
            fixture
                .contribute(&mut Contributions::__new(owner.clone(), &mut sink))
                .expect("the fixture contributes");
            AdmittedMember {
                secret_grants: Vec::new(),
                owner,
                metadata: fixture.metadata(),
                standing: StandingTag::Install,
                sink,
            }
        })
        .collect();
    Admitted::__from_facade(AdmittedParts {
        members,
        ..AdmittedParts::default()
    })
}
