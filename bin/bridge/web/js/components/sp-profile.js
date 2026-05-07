import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";

function fmtNumber(n) {
  if (n == null) { return "—"; }
  const v = Number(n);
  if (!Number.isFinite(v)) { return "—"; }
  if (v >= 1_000_000) { return `${(v / 1_000_000).toFixed(2)}M`; }
  if (v >= 1_000) { return `${(v / 1_000).toFixed(1)}k`; }
  return String(v);
}

function fmtUsd(microdollars) {
  if (microdollars == null) { return "—"; }
  const usd = Number(microdollars) / 1_000_000;
  if (!Number.isFinite(usd)) { return "—"; }
  if (usd >= 100) { return `$${usd.toFixed(0)}`; }
  if (usd >= 1) { return `$${usd.toFixed(2)}`; }
  if (usd >= 0.01) { return `$${usd.toFixed(3)}`; }
  return `$${usd.toFixed(5)}`;
}

function fmtDelta(curr, prev) {
  if (curr == null || prev == null || Number(prev) === 0) { return ""; }
  const c = Number(curr);
  const p = Number(prev);
  const pct = ((c - p) / p) * 100;
  if (!Number.isFinite(pct)) { return ""; }
  const sign = pct > 0 ? "+" : "";
  return `${sign}${pct.toFixed(0)}% vs prev`;
}

function fmtRelTime(iso) {
  if (!iso) { return "—"; }
  const t = Date.parse(iso);
  if (!Number.isFinite(t)) { return "—"; }
  const diffSec = Math.floor((Date.now() - t) / 1000);
  if (diffSec < 60) { return `${diffSec}s ago`; }
  if (diffSec < 3600) { return `${Math.floor(diffSec / 60)}m ago`; }
  if (diffSec < 86400) { return `${Math.floor(diffSec / 3600)}h ago`; }
  return `${Math.floor(diffSec / 86400)}d ago`;
}

function fmtExp(unix) {
  if (!unix) { return "—"; }
  const ms = Number(unix) * 1000;
  if (!Number.isFinite(ms)) { return "—"; }
  const date = new Date(ms);
  return date.toISOString().replace("T", " ").slice(0, 19) + " UTC";
}

function decodeJwtClaims(token) {
  if (!token || typeof token !== "string") { return null; }
  const parts = token.split(".");
  if (parts.length !== 3) { return null; }
  try {
    const padded = parts[1].replace(/-/g, "+").replace(/_/g, "/");
    const json = atob(padded + "===".slice((padded.length + 3) % 4));
    return JSON.parse(json);
  } catch (_) {
    return null;
  }
}

export class SpProfile extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
    this.profile = null;
    this.loading = true;
    this.error = "";
    this.registerAction("refresh", () => this._fetch());
  }

  onConnect() {
    bridge.stateSnapshot().then((s) => { this.snapshot = s; }).catch((e) => console.warn("snapshot failed", e));
    this.bridgeSubscribe("state.changed", (s) => {
      const wasSignedIn = this.snapshot && this.snapshot.verified_identity;
      const nowSignedIn = s && s.verified_identity;
      this.snapshot = s;
      if (!wasSignedIn && nowSignedIn) { this._fetch(); }
      if (wasSignedIn && !nowSignedIn) { this.profile = null; }
    });
    this._fetch();
  }

  async _fetch() {
    this.loading = true;
    this.error = "";
    try {
      this.profile = await bridge.profileFetch();
    } catch (e) {
      this.error = (e && e.message) || "profile fetch failed";
      this.profile = null;
    } finally {
      this.loading = false;
    }
  }

  render() {
    return `
      <header class="sp-tab__header">
        <h1 data-l10n-id="profile-heading">Profile</h1>
        <button class="sp-btn-ghost" type="button" data-action="refresh" data-l10n-id="profile-refresh">Refresh</button>
      </header>
      ${this._renderBody()}
    `;
  }

  _renderBody() {
    if (this.error) {
      return `
        <section class="sp-profile-error" role="alert">
          <p data-l10n-id="profile-error-fetch">Could not load profile.</p>
          <p class="sp-u-muted">${escapeHtml(this.error)}</p>
        </section>
      `;
    }
    if (this.loading && !this.profile) {
      return this._skeleton();
    }
    if (!this.profile) {
      return `<section class="sp-profile-empty"><p>Sign in to see your profile.</p></section>`;
    }
    return `
      <div class="sp-profile-grid">
        ${this._renderIdentity()}
        ${this._renderUsage()}
        ${this._renderModels()}
        ${this._renderConversations()}
        ${this._renderAgents()}
        ${this._renderPlan()}
      </div>
    `;
  }

  _renderIdentity() {
    const id = this.profile.identity || {};
    const snap = this.snapshot || {};
    const cached = snap.cached_token;
    const claims = decodeJwtClaims(cached && cached.preview);
    const issuer = claims && claims.iss;
    const rows = [
      ["email", id.email],
      ["name", id.display_name],
      ["user_id", id.user_id],
      ["tenant_id", id.tenant_id],
      ["provider", id.provider],
      ["roles", Array.isArray(id.roles) && id.roles.length ? id.roles.join(", ") : null],
      ["jwt issuer", issuer],
      ["jwt expires", fmtExp(id.exp_unix)],
      ["gateway", this.profile.gateway],
      ["token", cached ? `${cached.length} bytes · ttl ${cached.ttl_seconds}s` : "—"],
    ].filter(([, v]) => v != null && v !== "");
    return `
      <article class="sp-profile-card sp-profile-card--identity">
        <header>
          <h2 data-l10n-id="profile-section-identity">Identity</h2>
        </header>
        <dl class="sp-profile-dl">
          ${rows.map(([k, v]) => `<dt>${escapeHtml(k)}</dt><dd>${escapeHtml(String(v))}</dd>`).join("")}
        </dl>
      </article>
    `;
  }

  _renderUsage() {
    const u = this.profile.usage;
    if (!u) {
      return `
        <article class="sp-profile-card sp-profile-card--usage" data-state="empty">
          <header><h2 data-l10n-id="profile-section-usage">Token usage</h2></header>
          <p class="sp-u-muted">No usage reported yet.</p>
        </article>
      `;
    }
    const tile = (label, w) => {
      if (!w) { return ""; }
      const delta = fmtDelta(w.cost_microdollars, w.previous_cost_microdollars);
      return `
        <div class="sp-profile-tile">
          <div class="sp-profile-tile__label">${escapeHtml(label)}</div>
          <div class="sp-profile-tile__value">${escapeHtml(fmtUsd(w.cost_microdollars))}</div>
          <div class="sp-profile-tile__sub">${escapeHtml(fmtNumber(w.tokens))} tokens · ${escapeHtml(fmtNumber(w.requests))} req</div>
          ${delta ? `<div class="sp-profile-tile__delta">${escapeHtml(delta)}</div>` : ""}
        </div>
      `;
    };
    return `
      <article class="sp-profile-card sp-profile-card--usage">
        <header><h2 data-l10n-id="profile-section-usage">Token usage</h2></header>
        <div class="sp-profile-tiles">
          ${tile("24h", u.d1)}
          ${tile("7 days", u.d7)}
          ${tile("30 days", u.d30)}
        </div>
      </article>
    `;
  }

  _renderModels() {
    const top = (this.profile.usage && this.profile.usage.top_models) || [];
    if (top.length === 0) {
      return `
        <article class="sp-profile-card sp-profile-card--models" data-state="empty">
          <header><h2 data-l10n-id="profile-section-models">Favorite models</h2></header>
          <p class="sp-u-muted">No model usage in the last 30 days.</p>
        </article>
      `;
    }
    const rows = top.slice(0, 5).map((m, i) => `
      <li class="sp-profile-model" data-rank="${i + 1}">
        <span class="sp-profile-model__rank">#${i + 1}</span>
        <span class="sp-profile-model__name">${escapeHtml(m.model)}</span>
        <span class="sp-profile-model__share">${escapeHtml((Number(m.token_share || 0) * 100).toFixed(1))}%</span>
        <span class="sp-profile-model__tokens">${escapeHtml(fmtNumber(m.tokens))} tokens</span>
        <span class="sp-profile-model__cost">${escapeHtml(fmtUsd(m.cost_microdollars))}</span>
      </li>
    `).join("");
    return `
      <article class="sp-profile-card sp-profile-card--models">
        <header><h2 data-l10n-id="profile-section-models">Favorite models</h2></header>
        <ol class="sp-profile-models">${rows}</ol>
      </article>
    `;
  }

  _renderConversations() {
    const c = (this.profile.usage && this.profile.usage.conversations) || null;
    if (!c) {
      return `
        <article class="sp-profile-card sp-profile-card--conversations" data-state="empty">
          <header><h2 data-l10n-id="profile-section-conversations">Conversations</h2></header>
          <p class="sp-u-muted">No conversations recorded yet.</p>
        </article>
      `;
    }
    const groups = (label, arr) => `
      <div class="sp-profile-group">
        <div class="sp-profile-group__label">${escapeHtml(label)}</div>
        ${arr && arr.length ? `
          <ul class="sp-profile-group__list">
            ${arr.slice(0, 5).map((g) => `
              <li>
                <span class="sp-profile-group__name">${escapeHtml(g.name || "—")}</span>
                <span class="sp-profile-group__count">${escapeHtml(fmtNumber(g.conversations))} conv · ${escapeHtml(fmtNumber(g.ai_requests))} req</span>
              </li>
            `).join("")}
          </ul>
        ` : `<p class="sp-u-muted">none</p>`}
      </div>
    `;
    const recent = (c.recent || []).slice(0, 5).map((r) => `
      <li class="sp-profile-recent">
        <span class="sp-profile-recent__id">${escapeHtml(r.context_id.slice(0, 12))}</span>
        <span class="sp-profile-recent__model">${escapeHtml(r.model || "—")}</span>
        <span class="sp-profile-recent__agent">${escapeHtml(r.agent_name || "—")}</span>
        <span class="sp-profile-recent__count">${escapeHtml(fmtNumber(r.ai_requests))} req</span>
        <span class="sp-profile-recent__when">${escapeHtml(fmtRelTime(r.last_activity))}</span>
      </li>
    `).join("");
    return `
      <article class="sp-profile-card sp-profile-card--conversations">
        <header>
          <h2 data-l10n-id="profile-section-conversations">Conversations</h2>
          <span class="sp-profile-card__count">${escapeHtml(fmtNumber(c.total_conversations))} total · ${escapeHtml(fmtNumber(c.total_ai_requests))} requests</span>
        </header>
        <div class="sp-profile-groups">
          ${groups("By model", c.by_model)}
          ${groups("By agent", c.by_agent)}
        </div>
        ${recent ? `
          <div class="sp-profile-recent-wrap">
            <div class="sp-profile-group__label">Recent</div>
            <ul class="sp-profile-recents">${recent}</ul>
          </div>
        ` : ""}
      </article>
    `;
  }

  _renderAgents() {
    const a = this.profile.agents || { items: [], total: 0, enabled: 0 };
    const items = (a.items || []).map((it) => `
      <li class="sp-profile-agent" data-enabled="${it.enabled ? "true" : "false"}">
        <span class="sp-profile-agent__dot sp-dot ${it.enabled ? "sp-dot--ok" : "sp-dot--unknown"}"></span>
        <span class="sp-profile-agent__name">${escapeHtml(it.display_name || it.id)}</span>
        <span class="sp-profile-agent__state">${escapeHtml(it.host_running ? "running" : "idle")}</span>
      </li>
    `).join("");
    return `
      <article class="sp-profile-card sp-profile-card--agents">
        <header>
          <h2 data-l10n-id="profile-section-agents">Available agents</h2>
          <span class="sp-profile-card__count">${escapeHtml(String(a.enabled))}/${escapeHtml(String(a.total))} enabled</span>
        </header>
        <ul class="sp-profile-agents">${items}</ul>
      </article>
    `;
  }

  _renderPlan() {
    const bp = this.profile.bridge_profile;
    if (!bp) {
      return "";
    }
    const rows = [
      ["auth scheme", bp.auth_scheme],
      ["inference gateway", bp.inference_gateway_base_url],
      ["organization", bp.organization_uuid],
      ["allowed models", Array.isArray(bp.models) && bp.models.length ? `${bp.models.length} models` : null],
    ].filter(([, v]) => v != null && v !== "");
    return `
      <article class="sp-profile-card sp-profile-card--plan">
        <header><h2 data-l10n-id="profile-section-plan">Plan & gateway</h2></header>
        <dl class="sp-profile-dl">
          ${rows.map(([k, v]) => `<dt>${escapeHtml(k)}</dt><dd>${escapeHtml(String(v))}</dd>`).join("")}
        </dl>
        ${Array.isArray(bp.models) && bp.models.length
          ? `<details><summary>${escapeHtml(`${bp.models.length} allowed models`)}</summary><ul class="sp-profile-models-allowed">${bp.models.map((m) => `<li>${escapeHtml(m)}</li>`).join("")}</ul></details>`
          : ""}
      </article>
    `;
  }

  _skeleton() {
    return `
      <div class="sp-profile-grid">
        <article class="sp-profile-card" data-state="probing"><header><h2>Identity</h2></header><p class="sp-u-muted">loading…</p></article>
        <article class="sp-profile-card" data-state="probing"><header><h2>Token usage</h2></header><p class="sp-u-muted">loading…</p></article>
        <article class="sp-profile-card" data-state="probing"><header><h2>Conversations</h2></header><p class="sp-u-muted">loading…</p></article>
        <article class="sp-profile-card" data-state="probing"><header><h2>Available agents</h2></header><p class="sp-u-muted">loading…</p></article>
      </div>
    `;
  }
}

reactive(SpProfile.prototype, ["snapshot", "profile", "loading", "error"]);
customElements.define("sp-profile", SpProfile);
