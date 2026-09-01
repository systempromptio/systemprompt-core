import { escapeHtml } from "/assets/js/utils/escape.js";
import { fmtRelative } from "/assets/js/utils/format.js";
import {
  reachabilityView, identityView, cloudTokenSummary, cloudTokenDetail, canLogout,
} from "/assets/js/utils/cloud-views.js";

function renderCloudDetails(rows) {
  const items = rows.map(([k, v]) => `<dt>${escapeHtml(k)}</dt><dd>${escapeHtml(v)}</dd>`).join("");
  return `
    <details>
      <summary>Details</summary>
      <dl class="sp-kpi-card__details">${items}</dl>
    </details>
  `;
}

export function renderCloudReachCard(snap, recheckError) {
  const reach = reachabilityView(snap.gateway_status || {});
  const probedAt = fmtRelative(snap.last_probe_at_unix);
  const rows = [
    ["gateway", snap.gateway_url || "—"],
    reach.reason ? ["error", reach.reason] : null,
    ["last probe", probedAt],
  ].filter(Boolean);
  return `
    <article class="sp-kpi-card" data-state="${reach.tone}">
      <div class="sp-kpi-card__head">
        <span data-l10n-id="status-cloud-reach-label">Reachability</span>
        <span class="sp-dot ${reach.dot}" aria-hidden="true"></span>
      </div>
      <div class="sp-kpi-card__value">
        <span>${escapeHtml(reach.value)}</span>
        ${reach.unit ? `<span class="sp-kpi-card__unit">${escapeHtml(reach.unit)}</span>` : ""}
      </div>
      <div class="sp-kpi-card__label">${escapeHtml(reach.label)}</div>
      ${recheckError ? `<p class="sp-kpi-card__error">${escapeHtml(recheckError)}</p>` : ""}
      ${renderCloudDetails(rows)}
      <div class="sp-kpi-card__foot">
        <span class="sp-kpi-card__foot-meta">probed ${escapeHtml(probedAt)}</span>
        <button class="sp-btn-ghost" type="button" data-action="recheck" data-l10n-id="status-cloud-recheck">Re-check</button>
      </div>
    </article>
  `;
}

export function renderCloudIdentityCard(snap, logoutError) {
  const ident = identityView(snap);
  const id = snap.verified_identity || {};
  const rows = [
    ["user_id", id.user_id || "—"],
    ["tenant_id", id.tenant_id || "—"],
    ["token", cloudTokenDetail(snap)],
  ];
  return `
    <article class="sp-kpi-card" data-state="${ident.tone}">
      <div class="sp-kpi-card__head">
        <span data-l10n-id="status-cloud-identity-label">Identity</span>
        <span class="sp-dot ${ident.dot}" aria-hidden="true"></span>
      </div>
      <div class="sp-kpi-card__value sp-kpi-card__value--text${ident.muted ? " sp-kpi-card__value--muted" : ""}">
        <span>${escapeHtml(ident.value)}</span>
      </div>
      <div class="sp-kpi-card__label">${escapeHtml(ident.label)}</div>
      ${logoutError ? `<p class="sp-kpi-card__error">${escapeHtml(logoutError)}</p>` : ""}
      ${renderCloudDetails(rows)}
      <div class="sp-kpi-card__foot">
        <span class="sp-kpi-card__foot-meta">${escapeHtml(cloudTokenSummary(snap))}</span>
        ${canLogout(snap)
          ? `<button class="sp-btn-ghost" type="button" data-action="logout" data-l10n-id="status-cloud-logout">Log out</button>`
          : ""}
      </div>
    </article>
  `;
}

export function renderCloudSkeleton() {
  return `
    <div class="sp-kpi-grid">
      <article class="sp-kpi-card" data-state="probing">
        <div class="sp-kpi-card__head"><span>Reachability</span><span class="sp-dot sp-dot--probing" aria-hidden="true"></span></div>
        <div class="sp-kpi-card__value sp-kpi-card__value--muted"><span>…</span></div>
        <div class="sp-kpi-card__label">probing</div>
      </article>
      <article class="sp-kpi-card" data-state="probing">
        <div class="sp-kpi-card__head"><span>Identity</span><span class="sp-dot sp-dot--probing" aria-hidden="true"></span></div>
        <div class="sp-kpi-card__value sp-kpi-card__value--muted sp-kpi-card__value--text"><span>checking…</span></div>
        <div class="sp-kpi-card__label">verifying credentials</div>
      </article>
    </div>
  `;
}
