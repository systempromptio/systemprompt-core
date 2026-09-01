import { escapeHtml } from "/assets/js/utils/escape.js";
import { fmtDurationLong } from "/assets/js/utils/format.js";
import { t } from "/assets/js/i18n.js";
import { fmtUnixUtc, decodeJwtClaims, profileExtraRows } from "/assets/js/utils/profile-format.js";

function definitionList(rows) {
  return `<dl class="sp-profile-dl">
    ${rows.map(([k, v]) => `<dt>${escapeHtml(k)}</dt><dd>${escapeHtml(String(v))}</dd>`).join("")}
  </dl>`;
}

function tokenLine(cached) {
  if (!cached) { return null; }
  const ttl = fmtDurationLong(cached.ttl_seconds);
  return t("profile-token-value", { ttl }) || `expires in ${ttl}`;
}

export function renderProfileIdentity(profile, snapshot) {
  const id = profile.identity || {};
  const cached = (snapshot || {}).cached_token;
  const claims = decodeJwtClaims(cached && cached.preview);
  const rows = [
    [t("profile-id-email") || "Email", id.email],
    [t("profile-id-name") || "Name", id.display_name],
    [t("profile-id-user") || "User ID", id.user_id],
    [t("profile-id-tenant") || "Organization ID", id.tenant_id],
    [t("profile-id-provider") || "Signed in with", id.provider],
    [t("profile-id-roles") || "Roles", Array.isArray(id.roles) && id.roles.length ? id.roles.join(", ") : null],
    [t("profile-id-issuer") || "Issued by", claims && claims.iss],
    [t("profile-id-expires") || "Session expires", fmtUnixUtc(id.exp_unix)],
    [t("profile-id-gateway") || "Gateway", profile.gateway],
    [t("profile-id-token") || "Session token", tokenLine(cached)],
  ].filter(([, v]) => v != null && v !== "").concat(profileExtraRows(id.extra));
  return `
    <article class="sp-profile-card sp-profile-card--identity">
      <header>
        <h2 data-l10n-id="profile-section-identity">Identity</h2>
      </header>
      ${definitionList(rows)}
    </article>
  `;
}

export function renderProfilePlan(profile) {
  const bp = profile.bridge_profile;
  if (!bp) { return ""; }
  const models = Array.isArray(bp.models) ? bp.models : [];
  const rows = [
    [t("profile-plan-auth-scheme") || "Sign-in method", bp.auth_scheme],
    [t("profile-plan-gateway") || "Inference gateway", bp.inference_gateway_base_url],
    [t("profile-plan-organization") || "Organization", bp.organization_uuid],
    [t("profile-plan-models") || "Allowed models", models.length ? `${models.length} models` : null],
  ].filter(([, v]) => v != null && v !== "");
  const allowed = models.length
    ? `<details><summary>${escapeHtml(`${models.length} allowed models`)}</summary><ul class="sp-profile-models-allowed">${models.map((m) => `<li>${escapeHtml(m)}</li>`).join("")}</ul></details>`
    : "";
  return `
    <article class="sp-profile-card sp-profile-card--plan">
      <header><h2 data-l10n-id="profile-section-plan">Plan & gateway</h2></header>
      ${definitionList(rows)}
      ${allowed}
    </article>
  `;
}

// The modifier classes matter here as much as in the real render: without
// them the placeholders occupy one track each and the grid visibly relays out
// the moment the data lands.
export function renderProfileSkeleton() {
  const loading = `<p class="sp-u-muted" data-l10n-id="profile-loading">loading…</p>`;
  return `
    <div class="sp-profile-grid">
      <article class="sp-profile-card sp-profile-card--identity" data-state="probing"><header><h2 data-l10n-id="profile-section-identity">Identity</h2></header>${loading}</article>
      <article class="sp-profile-card sp-profile-card--usage" data-state="probing"><header><h2 data-l10n-id="profile-section-usage">Token usage</h2></header>${loading}</article>
      <article class="sp-profile-card sp-profile-card--models" data-state="probing"><header><h2 data-l10n-id="profile-section-models">Favorite models</h2></header>${loading}</article>
      <article class="sp-profile-card sp-profile-card--conversations" data-state="probing"><header><h2 data-l10n-id="profile-section-conversations">Conversations</h2></header>${loading}</article>
    </div>
  `;
}
