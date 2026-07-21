import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { fmtRelative, publishSectionState } from "/assets/js/utils/format.js";
import { logout } from "/assets/js/services/session-service.js";

function reachabilityView(status) {
  if (status.state === "reachable") {
    return { state: "ok", dot: "sp-dot--ok", value: String(status.latency_ms ?? "?"), unit: "ms", label: "reachable" };
  }
  if (status.state === "probing") {
    return { state: "probing", dot: "sp-dot--probing", value: "…", unit: "", label: "probing" };
  }
  if (status.state === "unreachable") {
    return { state: "err", dot: "sp-dot--err", value: "—", unit: "", label: "unreachable", reason: status.reason || "unknown error" };
  }
  return { state: "unknown", dot: "sp-dot--unknown", value: "—", unit: "", label: "unknown" };
}

function identityView(snap, reachable) {
  const id = snap.verified_identity;
  if (!reachable) {
    return { state: "unknown", dot: "sp-dot--unknown", value: "—", label: "gateway unreachable", muted: true };
  }
  if (id && (id.email || id.user_id)) {
    return { state: "ok", dot: "sp-dot--ok", value: id.email || id.user_id, label: "signed in", muted: false };
  }
  if (snap.pat_present) {
    return { state: "probing", dot: "sp-dot--probing", value: "…", label: "verifying credentials", muted: true };
  }
  return { state: "warn", dot: "sp-dot--warn", value: "—", label: "not signed in", muted: true };
}

function rollUp(a, b) {
  const order = { err: 4, warn: 3, probing: 2, unknown: 1, ok: 0 };
  return (order[a] >= order[b]) ? a : b;
}

function sectionLabel(state) {
  if (state === "ok") { return "healthy"; }
  if (state === "warn") { return "attention"; }
  if (state === "err") { return "down"; }
  if (state === "probing") { return "checking…"; }
  return "unknown";
}

export class SpCloudStatus extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
    this.recheckError = "";
    this.logoutError = "";
    this.registerAction("recheck", () => this._onRecheck());
    this.registerAction("logout", () => this._onLogout());
  }

  onConnect() {
    bridge.stateSnapshot().then((snap) => { this.snapshot = snap; }).catch((e) => {
      console.error("sp-cloud-status snapshot failed", e);
    });
    this.bridgeSubscribe("state.changed", (snap) => { this.snapshot = snap; });
    this.bridgeSubscribe("gateway.changed", () => {
      bridge.stateSnapshot().then((snap) => { this.snapshot = snap; }).catch((e) => console.warn("snapshot failed", e));
    });
  }

  async _onRecheck() {
    this.recheckError = "";
    try { await bridge.gatewayProbe(); }
    catch (e) { this.recheckError = (e && e.message) || "probe failed"; }
  }

  async _onLogout() {
    this.logoutError = await logout();
  }

  render() {
    const snap = this.snapshot;
    if (!snap) {
      return this._skeleton();
    }
    const status = snap.gateway_status || { state: "unknown" };
    const reach = reachabilityView(status);
    const ident = identityView(snap, status.state === "reachable");
    const id = snap.verified_identity || {};
    const tokenPrimary = snap.cached_token
      ? `JWT · ${snap.cached_token.ttl_seconds}s`
      : (snap.pat_present ? "PAT stored" : "no token");
    const gw = snap.gateway_url || "—";
    const probedAt = fmtRelative(snap.last_probe_at_unix);

    const reachDetailsRows = [
      ["gateway", escapeHtml(gw)],
      reach.reason ? ["error", escapeHtml(reach.reason)] : null,
      ["last probe", escapeHtml(probedAt)],
    ].filter(Boolean);

    const identDetailsRows = [
      ["user_id",   escapeHtml(id.user_id   || "—")],
      ["tenant_id", escapeHtml(id.tenant_id || "—")],
      snap.cached_token
        ? ["token", `JWT · ${snap.cached_token.length} bytes · ttl ${snap.cached_token.ttl_seconds}s`]
        : (snap.pat_present
            ? ["token", "PAT stored — JWT will refresh on next probe"]
            : ["token", "none"]),
    ];

    return `
      <div class="sp-kpi-grid">
        <article class="sp-kpi-card" data-state="${reach.state}">
          <div class="sp-kpi-card__head">
            <span data-l10n-id="status-cloud-reach-label">Reachability</span>
            <span class="sp-dot ${reach.dot}" aria-hidden="true"></span>
          </div>
          <div class="sp-kpi-card__value">
            <span>${escapeHtml(reach.value)}</span>
            ${reach.unit ? `<span class="sp-kpi-card__unit">${escapeHtml(reach.unit)}</span>` : ""}
          </div>
          <div class="sp-kpi-card__label">${escapeHtml(reach.label)}</div>
          ${this.recheckError ? `<p class="sp-kpi-card__error">${escapeHtml(this.recheckError)}</p>` : ""}
          <details>
            <summary>Details</summary>
            <dl class="sp-kpi-card__details">
              ${reachDetailsRows.map(([k, v]) => `<dt>${escapeHtml(k)}</dt><dd>${v}</dd>`).join("")}
            </dl>
          </details>
          <div class="sp-kpi-card__foot">
            <span class="sp-kpi-card__foot-meta">probed ${escapeHtml(probedAt)}</span>
            <button class="sp-btn-ghost" type="button" data-action="recheck" data-l10n-id="status-cloud-recheck">Re-check</button>
          </div>
        </article>

        <article class="sp-kpi-card" data-state="${ident.state}">
          <div class="sp-kpi-card__head">
            <span data-l10n-id="status-cloud-identity-label">Identity</span>
            <span class="sp-dot ${ident.dot}" aria-hidden="true"></span>
          </div>
          <div class="sp-kpi-card__value sp-kpi-card__value--text${ident.muted ? " sp-kpi-card__value--muted" : ""}">
            <span>${escapeHtml(ident.value)}</span>
          </div>
          <div class="sp-kpi-card__label">${escapeHtml(ident.label)}</div>
          ${this.logoutError ? `<p class="sp-kpi-card__error">${escapeHtml(this.logoutError)}</p>` : ""}
          <details>
            <summary>Details</summary>
            <dl class="sp-kpi-card__details">
              ${identDetailsRows.map(([k, v]) => `<dt>${escapeHtml(k)}</dt><dd>${typeof v === "string" ? escapeHtml(v) : v}</dd>`).join("")}
            </dl>
          </details>
          <div class="sp-kpi-card__foot">
            <span class="sp-kpi-card__foot-meta">${escapeHtml(tokenPrimary)}</span>
            ${id.email || id.user_id || snap.pat_present
              ? `<button class="sp-btn-ghost" type="button" data-action="logout" data-l10n-id="status-cloud-logout">Log out</button>`
              : ""}
          </div>
        </article>
      </div>
    `;
  }

  afterRender() {
    const snap = this.snapshot;
    if (!snap) {
      publishSectionState(this, "probing", "checking…");
      return;
    }
    const status = snap.gateway_status || { state: "unknown" };
    const reach = reachabilityView(status);
    const ident = identityView(snap, status.state === "reachable");
    const overall = rollUp(reach.state, ident.state);
    publishSectionState(this, overall, sectionLabel(overall));
  }

  _skeleton() {
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
}

reactive(SpCloudStatus.prototype, ["snapshot", "recheckError", "logoutError"]);
customElements.define("sp-cloud-status", SpCloudStatus);
