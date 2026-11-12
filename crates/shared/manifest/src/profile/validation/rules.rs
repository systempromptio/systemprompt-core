//! The profile validation rules and the key paths each one reads.
//!
//! [`Profile::validate`] runs every rule. The management view runs a rule
//! only when none of its inputs holds a value whose variable is unset on this
//! host, and reports the rest as not evaluated; both walk this one table, so
//! there is a single owner of every rule. A rule that reads a field derived
//! from `server.external_url` lists the derivation's inputs
//! ([`DERIVE_INPUTS`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::profile::Profile;

#[derive(Debug)]
pub(crate) struct ProfileRule {
    pub(crate) name: &'static str,
    pub(crate) inputs: &'static [&'static str],
    pub(crate) check: fn(&Profile, &mut Vec<String>),
}

pub(crate) const DERIVE_INPUTS: &[&str] = &[
    "server.external_url",
    "server.host",
    "server.port",
    "server.extra_cors_origins",
    "governance",
];

pub(crate) const RULES: &[ProfileRule] = &[
    ProfileRule {
        name: "required-fields",
        inputs: &[
            "name",
            "display_name",
            "site.name",
            "server.host",
            "server.external_url",
            "server.port",
            "server.metrics_port",
        ],
        check: |p, e| p.validate_required_fields(e),
    },
    ProfileRule {
        name: "urls",
        inputs: &[
            "server.external_url",
            "server.host",
            "server.port",
            "server.extra_cors_origins",
            "governance",
            "security.trusted_issuers",
        ],
        check: |p, e| p.validate_urls(e),
    },
    ProfileRule {
        name: "paths",
        inputs: &["target", "paths"],
        check: |p, e| p.validate_paths(e, p.target.is_cloud()),
    },
    ProfileRule {
        name: "security-settings",
        inputs: &[
            "security.jwt_access_token_expiration",
            "security.jwt_refresh_token_expiration",
            "security.api_keys",
        ],
        check: |p, e| p.validate_security_settings(e),
    },
    ProfileRule {
        name: "database-pool",
        inputs: &["database.pool"],
        check: |p, e| p.validate_database_pool(e),
    },
    ProfileRule {
        name: "trusted-proxies",
        inputs: &["target", "server.trusted_proxies"],
        check: |p, e| p.validate_trusted_proxies(e, p.target.is_cloud()),
    },
    ProfileRule {
        name: "instance-identity",
        inputs: &["target", "server.instance_id"],
        check: |p, e| p.validate_instance_identity(e, p.target.is_cloud()),
    },
    ProfileRule {
        name: "role",
        inputs: &["server.role", "server.advertise_url", "storage"],
        check: |p, e| p.validate_role(e),
    },
    ProfileRule {
        name: "shutdown",
        inputs: &["server.shutdown"],
        check: |p, e| p.validate_shutdown(e),
    },
    ProfileRule {
        name: "cors-origins",
        inputs: DERIVE_INPUTS,
        check: |p, e| p.validate_cors_origins(e),
    },
    ProfileRule {
        name: "rate-limits",
        inputs: &["rate_limits"],
        check: |p, e| p.validate_rate_limits(e),
    },
    ProfileRule {
        name: "governance",
        inputs: &[
            "target",
            "server.external_url",
            "server.host",
            "server.port",
            "server.extra_cors_origins",
            "governance",
        ],
        check: |p, e| p.validate_governance(e, p.target.is_cloud()),
    },
    ProfileRule {
        name: "storage",
        inputs: &["storage", "paths.data", "paths.storage"],
        check: |p, e| p.validate_storage(e),
    },
    ProfileRule {
        name: "external-url-reachable",
        inputs: &[
            "target",
            "server.external_url",
            "server.host",
            "server.port",
            "server.extra_cors_origins",
            "governance",
        ],
        check: |p, e| p.validate_external_url_is_reachable(e, p.target.is_cloud()),
    },
    ProfileRule {
        name: "services-sources",
        inputs: &["target", "services"],
        check: |p, e| p.validate_services_sources(e, p.target.is_cloud()),
    },
    ProfileRule {
        name: "secrets",
        inputs: &["secrets"],
        check: |p, e| p.validate_secrets(e),
    },
    ProfileRule {
        name: "observability",
        inputs: &["observability"],
        check: |p, e| p.validate_observability(e),
    },
    ProfileRule {
        name: "config-planes",
        inputs: &["config_planes"],
        check: |p, e| p.validate_config_planes(e),
    },
    ProfileRule {
        name: "licence",
        inputs: &["licence"],
        check: |p, e| p.validate_licence(e),
    },
    ProfileRule {
        name: "cloud-deploy",
        inputs: &["cloud"],
        check: |p, e| p.validate_cloud_deploy(e),
    },
];
