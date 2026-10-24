# Golden catalog snapshots (core 0.72.1)

What the unmodified 0.72.1 catalog resolves every model id to, recorded so the
0.73 config rewrite (credential.md §9.1) can be proven behaviour-preserving.

```
<fixture>/legacy/ai/      the input services/ai tree, verbatim (secrets by name only)
<fixture>/golden/resolution.json
```

| fixture        | source                                                      |
|----------------|-------------------------------------------------------------|
| `core-default` | `crates/shared/manifest/src/services/providers/default_catalog.yaml` at 0.72.1, no gateway |
| `byok-gateway` | Gateway with personal (BYOK) credentials and OpenCode Go; recorded from a production services/ai tree, names neutralised |
| `vertex-multiregion` | Claude and Gemini on Vertex AI across global and regional endpoints, with API-key failovers and per-scope chains; names neutralised |

Recorder: `crates/tests/unit/app/gateway-golden/src/golden_catalog/`, run with
`cargo test -p systemprompt-gateway-golden-tests -- --ignored record_golden_catalog`.
It copies `legacy/ai` into a temp `services/` with a synthetic
`config/config.yaml` that includes only the ai files, loads it through
`ConfigLoader::load_from_path` (includes, gateway projection, validation), and
boots no secret store, so no secret is read and no provider is demoted for a
missing credential (that is health state, not catalog).

## Schema

Top level: `core_version`, `fixture`, `gateway_enabled`, `default_model`,
`models`. Every object has its keys sorted; `models` is keyed by requested id
(every provider model id and alias, plus `gateway.default_model`), sorted.

Per requested id:

| key | meaning |
|-----|---------|
| `exposed` | `GatewayConfig::is_model_exposed` (or registry membership with no gateway) |
| `hidden` | any serving catalog entry has `hidden: true` |
| `advertised_to` | client surfaces (`anthropic`, `openai`, `gemini`) whose `advertised_model_ids` list contains the id; `[]` = not advertised. Aliases are never advertised |
| `strategy` | the matched route's selection strategy |
| `chain` | ordered deployment hops for a plain request (no tools, no scope attribution): primary first, then failovers |
| `context_fallbacks` | hops used when the request exceeds the primary's context window (present only if declared) |
| `by_scope` | `{dimension, unmapped, chains: {value: [hops]}}` when the route maps scope values |
| `error` | why the id did not resolve (`no route`, `no provider serves the model`) |

Each hop:

| key | meaning |
|-----|---------|
| `url` | the endpoint as configured (`{project}` placeholders unfilled: they come from the credential) |
| `wire` | outbound wire protocol |
| `credential` | `{source: platform, secret: <secret name>}` or `{source: personal, owner: <credential owner>}`. In 0.72.1 the personal owner is the provider id `credential_from` names; 0.73 maps it to a credential id |
| `upstream_model` | `ProviderEntry::upstream_model_for(route.upstream_model, requested)`, the model name sent upstream |
| `served` | `find_served_model` finds a catalog entry on this hop's provider |
| `pricing` | `systemprompt_gateway::pricing::resolve_upstream` for the hop (route override, else served model), per million tokens; `{error}` if missing |
| `weight` | the deployment weight, if any |
| `info` | informational only, never identity: `provider` name and `route_id` |

Compare on everything except `info`.

## Coverage: plain requests only

Every id is resolved for a plain request: no tools, no scope attribution, no
route selectors. A `when:` route that only a tool-carrying request matches,
a `by_scope` chain chosen by attribution (its chains are listed under
`by_scope` but never selected), and selector refinement are not exercised.
The 0.73 golden test proves parity for this coverage only.
