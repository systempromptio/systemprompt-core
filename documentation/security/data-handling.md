# Data Handling

What data a self-hosted systemprompt deployment processes, where that data can go, and how long
it is kept. Addresses ISO/IEC 27001:2022 A.5.12, A.5.33–5.34, A.8.10–8.12 and A.8.24.

## 1. Vendor access

None. The software runs in the customer's infrastructure, and the vendor receives no telemetry,
logs, crash reports, usage or licence data from it ([Security policy § Telemetry](../../SECURITY.md#telemetry)).
The vendor is not a processor of customer personal data for a self-hosted deployment.

## 2. Data the software stores

All data is held in the customer's PostgreSQL database and configured file storage.

| Category | Examples | Notes |
|----------|----------|-------|
| Identity | User records, OAuth clients, sessions, WebAuthn credentials | Users come from the customer's IdP |
| Governance and audit | Policy decisions, gateway request records, structured logs | Can contain prompts, responses and tool payloads on enabled paths |
| Content | Documents, files, agent and task state | As created by users |
| Analytics | Request and session analytics, engagement events, fingerprint reputation (last IP address and user agent) | Collected inside the deployment; personal-data tables have retention windows (§4) |
| Organisational memory | Facts, recalls and results | A user-scoped fact is visible only under its owner's key; topic audiences never widen it |
| Notifications | Outbox events, per-recipient rows, delivery failures | Delivery is at least once: a crash before delivery is recorded leads to re-delivery |
| Secrets | Provider API keys, JWT signing key | Loaded from a customer-managed secrets file or secrets source; never stored in the database by default |

## 3. Where data can leave the deployment

Only through channels the operator configures:

1. Upstream AI providers declared in the services manifest.
2. An OpenTelemetry endpoint, if one is configured.
3. systemprompt.io cloud, only when a cloud profile or the `cloud` commands are used.
4. A bridge release repository, if one is declared.

Each outbound request passes the SSRF guards described in
[Outbound egress controls](outbound-egress-controls.md). Network egress policy remains a
deployment control.

## 4. Retention and deletion

Retention is configured in the deployment profile (`crates/shared/manifest/src/profile/retention.rs`)
and enforced by scheduled jobs, including governance-decision retention
(`crates/infra/governance/src/decision_retention.rs`) and knowledge-store retention
(`crates/domain/brain/src/jobs/retention.rs`). The scheduler's `database_cleanup` job enforces the profile's `retention` windows; every window
has a default, so a profile that sets none still has bounded tables. For personal data in
analytics, `engagement_events_days` (default 90) bounds engagement events, and
`fingerprint_reputation_days` (default 90) deletes an unflagged fingerprint's reputation row,
with its last IP address and user agent, once it has not been seen for the window; a flagged
fingerprint is kept as enforcement memory with its IP address and user agent cleared. Raw
gateway request and response bodies are cleared after `ai_request_payload_raw_days` (default 7).
See the [configuration reference](../reference/configuration.md#retention). Per-user purge removes a user's rows across the
schema inventory (`crates/domain/users/src/repository/user/purge.rs`). Privacy-restricted reporting
views limit what reporting roles can see.

## 5. Encryption

| Layer | Responsibility |
|-------|----------------|
| In transit (ingress) | Customer: TLS termination at the reverse proxy ([Production deployment](../guides/deploy-production.md)) |
| In transit (outbound) | HTTPS to provider endpoints, using the customer-supplied trust store |
| At rest (database, volumes, backups) | Customer: storage or database encryption |
| Secrets | Customer: envelope encryption of the secrets file (Vault, cloud KMS, sops) ([Vault secrets](../guides/vault-secrets.md)) |
| Tokens | RS256-signed JWTs; signing key supplied by the customer |
