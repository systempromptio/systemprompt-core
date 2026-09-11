# Outbound Egress Controls

How systemprompt-core decides whether an outbound HTTP request may be made, and what a deployment still has to provide. This complements the SSRF row of the [threat model](threat-model.md) with the mechanism itself.

## 1. Why two layers

A URL string carries a scheme and a host, but not an address. `https://169.254.169.254/` names a link-local address and can be refused on sight; `https://metadata.example.com/` names nothing until DNS answers, and the answer may be `169.254.169.254`. A single parse-time check therefore cannot be the enforcement point. Core applies two layers with deliberately different reach.

### 1.1 Parse-time guard

`validate_outbound_url` and `validate_outbound_url_with_trust` in `crates/shared/models/src/net/mod.rs` run before a request is built. They refuse:

- any scheme other than `https`, or `http` to a non-loopback host that is not an explicitly trusted host;
- a URL with no host;
- a literal IP address in a blocked range: RFC 1918, loopback, link-local `169.254.0.0/16`, carrier-grade NAT `100.64.0.0/10`, unspecified and broadcast, IPv6 loopback and unspecified, `fe80::/10`, `fc00::/7`, and IPv4-mapped IPv6 forms of all of the above.

A public-looking hostname passes this layer by design. It is a pre-filter.

### 1.2 Connect-time guard

`guarded_client` in `crates/shared/models/src/net/client.rs` builds a `reqwest` client with two additions:

- a DNS resolver that filters every address a name resolves to through the same block list, refusing the connection if any resolved address is blocked. It runs for the initial request and for every redirect hop, because each hop connects afresh;
- a redirect policy that re-runs the parse-time guard on each hop, so a hop to a blocked literal or a downgrade to plain `http` is refused before a socket is opened, and that caps the number of hops.

A refusal surfaces to the caller as a typed `GuardedConnectError` naming the host and the refused address, and logs `Refused outbound connection to blocked address` with the host and address as structured fields.

## 2. Exemptions

- **Trusted hosts** — `SYSTEMPROMPT_TRUSTED_HTTP_HOSTS`, a comma-separated list of hostnames. A host named there may use plain `http` and may resolve into a blocked range. This is the operator's escape hatch for an internal inference server or tool server on a sealed network. Matching is exact and case-insensitive on the hostname; it is not a suffix or CIDR match.
- **Loopback** — `localhost` and literal loopback addresses are permitted by default for local development. Surfaces whose URL is chosen by the caller of an inference request, rather than by the operator, disable this exemption.

## 3. Which surface uses which layer

| Surface | Who chooses the URL | Guard |
|---|---|---|
| Gateway image fetch (Gemini URL images) | the inference caller | connect-time, loopback denied |
| Slack reply to `response_url` | Slack payload | connect-time |
| Teams reply to `serviceUrl` | Teams payload | connect-time |
| External MCP servers and their OAuth metadata | agent configuration and the remote server | connect-time |
| Governance webhooks | operator or dashboard | connect-time, plus parse-time per send |
| Authz hook, JWKS, client-metadata fetch | profile and OAuth discovery | connect-time |
| Teams token and OpenID endpoints | profile | parse-time; operator-configured constants |
| Provider base URLs, bundle sources, Vault | profile | parse-time; operator-configured |

The rule: any URL a request caller can influence goes through the connect-time guard. Operator-configured URLs may use the parse-time guard alone, because the operator is already trusted to point the platform at their own infrastructure.

## 4. What the deployment still owns

The guard prevents the platform from being used as a proxy into its own network. It does not replace network egress policy:

- **Egress allow-lists** — a firewall or service mesh that restricts which destinations the binary may reach is the primary control. The guard is defence in depth against a mis-scoped allow-list.
- **Cloud metadata services** — on platforms with an instance metadata service, block it at the network layer as well. On Fly.io there is no IMDS on `169.254.169.254`; the platform's internal services live in `fdaa::/16`, which is inside the `fc00::/7` range the guard refuses.
- **Trusted-host hygiene** — every entry in `SYSTEMPROMPT_TRUSTED_HTTP_HOSTS` is a hole in the block list. Keep it to the hosts you operate.

## 5. Verification

`crates/tests/unit/shared/models/src/net.rs` covers the block list, a hostname resolving into a blocked range, redirect chains ending in a blocked range, an `https` to `http` downgrade hop, and the hop cap. `crates/tests/unit/entry/api/src/services/gateway/image_fetch.rs` and `crates/tests/unit/domain/mcp/src/services/client/http_context.rs` drive the gateway and MCP surfaces end to end against a mock redirector and assert the blocked target receives no request. `just test-ssrf-live` runs the real-DNS cases against `169.254.169.254.nip.io` and `metadata.google.internal`.
