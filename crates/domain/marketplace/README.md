# systemprompt-marketplace

Per-user marketplace filtering for systemprompt.io.

Defines the [`MarketplaceFilter`] trait, which the gateway's
`/v1/bridge/manifest` handler invokes to decide which plugins, skills,
agents, and managed MCP servers a given user is permitted to see. The
filter runs **before** the manifest is signed, so the Ed25519 signature
covers exactly the set the user is authorised for.

The crate ships [`AllowAllFilter`] as a passthrough default so core can
serve unfiltered manifests when no extension provides a policy.
Deployments that need ACL — typically the `systemprompt-template`
extension — register their own `Arc<dyn MarketplaceFilter>` on the
`AppContext`.
