# Loading secrets from Vault

How to point a deployment at HashiCorp Vault or OpenBao instead of a secrets file, using KV v2 and one of three auth methods, so rotating a credential is a restart rather than a redeploy.

By default the binary reads a JSON secrets document from disk, or from the environment on a deployment host. Setting `secrets.source: vault` makes it fetch that same document from a KV v2 path at boot. Nothing else changes: the document shape, the validation and every consumer are identical.

Rotation is a process restart. Live refresh without a restart is not implemented.

## Prerequisites

- Vault or OpenBao with a KV v2 mount the instance can read.
- A way for the process to authenticate: a token, an AppRole role and secret id, or a Kubernetes service account.
- The `systemprompt` CLI, for seeding and checking.

## 1. Configure the profile

```yaml
secrets:
  source: vault
  validation: warn              # strict | warn | skip
  vault:
    address: https://vault.example.com
    mount: secret               # default: secret
    path: systemprompt/production
    namespace: engineering      # Vault Enterprise / OpenBao namespaces; optional
    ca_cert_path: /etc/ssl/private-ca.pem
    timeout_secs: 10            # default: 10, maximum 120
    retries: 3                  # default: 3, maximum 10
    auth:
      method: approle
      role_id_env: VAULT_ROLE_ID
      secret_id_env: VAULT_SECRET_ID
      mount: approle
    keys:
      manifest_signing_seed:
        path: systemprompt/shared
        field: manifest_signing_seed
```

`source: vault` requires the `vault:` block, and the other two sources refuse it: the profile fails to parse rather than silently ignoring a block that does nothing. `secrets_path` is meaningless here and is not read.

There is deliberately **no TLS-verification escape hatch**. The block denies unknown fields, so a `skip_verify:` key is a parse error rather than a quietly downgraded connection. A private CA goes in `ca_cert_path`.

### Auth methods

`auth.method` selects one of three, and each names where its credentials come from rather than carrying them.

```yaml
auth:
  method: token
  token_env: VAULT_TOKEN        # default
  token_file: /run/secrets/vault-token   # optional; either source works
```

```yaml
auth:
  method: approle
  role_id_env: VAULT_ROLE_ID        # default
  secret_id_env: VAULT_SECRET_ID    # default
  mount: approle                    # default
```

```yaml
auth:
  method: kubernetes
  role: systemprompt
  jwt_path: /var/run/secrets/kubernetes.io/serviceaccount/token   # default
  mount: kubernetes                                               # default
```

AppRole and Kubernetes log in and receive a short-lived client token. Whatever the method, the token is held in zeroizing memory for the length of the fetch and is never logged, never stored, and never placed in an error.

### Never put a token in YAML

`${VAR}` interpolation works in `profile.yaml`, which means `token: ${VAULT_TOKEN}` would parse. Do not write it. There is no field for it, and the token is deliberately reachable only through the environment, a file, or a login. A profile is a configuration document that gets committed, copied and attached to tickets; a token in it is a token in all of those places.

## 2. Seed the document

One KV v2 document holds the whole `secrets.json` shape. The simplest path is to put the file you already have straight in:

```bash
vault kv put -mount=secret systemprompt/production @secrets.json
```

Read it back to confirm the field names survived:

```bash
vault kv get -mount=secret -format=json secret/systemprompt/production
```

The policy the instance needs is a read on the data path, plus a read on any path named in `keys:`:

```hcl
path "secret/data/systemprompt/production" { capabilities = ["read"] }
path "secret/data/systemprompt/shared"     { capabilities = ["read"] }
```

### Shared identity material

`keys:` redirects individual entries to a different KV path and field, after the base document is read. Each entry names a key in the secrets document, the KV path to read, and the field to take from it.

That is how material shared across instances lives in one place: every instance keeps its own document for its own database URL and API keys, while the manifest signing seed is read from one shared path. A `keys:` entry naming a field that does not exist is an error, not an omission — a missing shared seed would otherwise surface much later as unverifiable manifests.

## 3. Understand the precedence

The source is decided by five rules, highest first. Nothing about the decision reads the network or the filesystem; it is a function of the profile and three observed facts.

| # | Condition | Source |
|---|-----------|--------|
| 1 | The process is a subprocess and a valid pepper is already in its environment | environment |
| 2 | `secrets.source: vault` | Vault, **including on deployment hosts** |
| 3 | A deployment host, with a valid pepper or `source: env` | environment |
| 4 | `source: env` running locally | the file, then the environment |
| 5 | `source: file` | the file |

Rule 2 sitting above rule 3 is the important one. On a deployment host the environment normally wins, because that is how a platform injects secrets. A Vault source overrides that, so a container is never silently downgraded to whatever the host happens to carry.

Rule 1 is why child processes never talk to Vault. The parent fetches once and hands its children the resolved secrets as environment variables after clearing their environment, so an MCP server started by the platform inherits no `VAULT_TOKEN`, no role id and no secret id. Only the parent holds Vault credentials, and only for as long as the fetch takes.

## 4. Fail-closed

A failed Vault fetch aborts the boot. This holds under every `validation` mode, including `skip`: `validation` governs how a *loaded* document is checked, not whether the source may be abandoned.

There is no environment fallback. Falling back would start the process on whatever stale credentials the host happened to carry — most likely the previous deployment's — and it would start successfully, which is the worst possible outcome. An unreachable Vault is an outage that says so.

The retry budget in `retries` covers connect failures, 5xx responses and 429s. An authentication failure or a missing path is not retried; those do not get better by asking again.

Errors from this path never carry a response body or a token. They name the address, mount, path and auth method, which is what an operator needs, and nothing that would leak into a log aggregator.

## 5. Network requirements

The address goes through the same outbound-URL guard as every other outbound request, which blocks private and link-local literals by default. An in-cluster Vault reached over plaintext at a private address is refused as written.

Two ways through, in order of preference:

1. Reach Vault by DNS name over HTTPS, which is what the guard expects and what you want anyway.
2. If you must use a private literal or plaintext, add the host to `SYSTEMPROMPT_TRUSTED_HTTP_HOSTS`, the same comma-separated allowlist that governs other trusted in-cluster endpoints.

Redirects are not followed. A Vault address that answers with a redirect is a misconfiguration, and following it would move a token to an unverified host.

## 6. Check it

```bash
systemprompt admin config secret check
```

This reports the source kind and, for a Vault source, its address, mount, path and auth method, then the key names present and any required key that is missing. It never prints a value, and it does not report the KV document version.

Use it to confirm the precedence resolved the way you expect before you deploy, and to distinguish "Vault is unreachable" from "Vault is fine but the document is missing a key".

## 7. Deploying with a Vault source

Deployment pushes **bootstrap credentials only**. A Vault-sourced instance never receives `secrets.json` and never receives the manifest signing key; it receives the address and the role and secret ids, and fetches everything else itself.

That shrinks the deploy-time secret surface to the credential that can fetch secrets, which is the point of the exercise: a developer machine no longer needs to hold the production secrets document in order to deploy.

`cloud doctor` gains a Vault preflight: it checks the address is reachable and well-formed, that the configured auth method succeeds, and that the document is readable with the key names the profile expects. Deployment runs the doctor first, so a Vault misconfiguration is caught before anything is pushed rather than at the first boot of the new release.

## Verify

```bash
systemprompt admin config secret check
systemprompt cloud doctor
```

A clean `secret check` naming `vault` as the source, with no missing keys, means the instance will boot. Because the path is fail-closed, a successful boot is itself proof the fetch worked; there is no degraded mode to mistake for success.

## Related pages

- [configure.md](configure.md) — the rest of the profile.
- [services-bundles.md](services-bundles.md) — the matching treatment for the services tree.
- [deploy-production.md](deploy-production.md) — key rotation and disaster recovery.
- [../reference/configuration.md](../reference/configuration.md) — the `secrets:` key schema.
