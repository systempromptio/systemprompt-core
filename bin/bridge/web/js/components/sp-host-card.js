import { html, nothing } from "/assets/js/vendor/lit-all.js";
import { BridgeElement } from "/assets/js/components/base.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";

function chooseBadge(installed, partial, proxyState) {
  if (!installed) { return { text: t("host-badge-not-installed") || "not installed", cls: "sp-badge--warn" }; }
  if (partial) { return { text: t("host-badge-partial") || "partial", cls: "sp-badge--warn" }; }
  if (proxyState === "Unconfigured") { return { text: t("host-badge-awaiting") || "awaiting first launch", cls: "sp-badge--warn" }; }
  if (proxyState === "Listening") { return { text: t("host-badge-healthy") || "healthy", cls: "sp-badge--ok" }; }
  return { text: t("host-badge-proxy-down") || "proxy down", cls: "sp-badge--err" };
}

export class SpHostCard extends BridgeElement {
  static properties = { host: { attribute: false }, snapshot: { attribute: false } };

  constructor() {
    super();
    this.host = null;
    this.snapshot = null;
  }

  createRenderRoot() { return this; }

  async _generate() {
    if (!this.host) { return; }
    try { await bridge.hostProfileGenerate(this.host.id); } catch (e) { console.warn("generate", e); }
  }
  async _install() {
    if (!this.host || !this.host.last_generated_profile) { return; }
    try { await bridge.hostProfileInstall(this.host.id, this.host.last_generated_profile); } catch (e) { console.warn("install", e); }
  }
  async _reverify() {
    if (!this.host) { return; }
    try { await bridge.hostProbe(this.host.id); } catch (e) { console.warn("reverify", e); }
  }

  render() {
    const host = this.host || {};
    const snap = this.snapshot || {};
    const hs = host.snapshot || null;
    const profileState = (hs && hs.profile_state) || { kind: "absent" };
    const missing = profileState.missing_required || [];
    const installed = profileState.kind === "installed";
    const partial = profileState.kind === "partial";
    const proxyState = ((snap.local_proxy && snap.local_proxy.state) || "Unknown").toString();
    const badge = hs ? chooseBadge(installed, partial, proxyState) : { text: "probing…", cls: "sp-badge--muted" };

    let profileDot = "sp-dot--err";
    let profileText = t("host-profile-not-installed") || "not installed";
    if (installed) { profileDot = "sp-dot--ok"; profileText = t("host-profile-installed") || "installed"; }
    else if (partial) { profileDot = "sp-dot--warn"; profileText = t("host-profile-partial", { missing: missing.join(", ") }) || `partial (${missing.join(", ")})`; }

    const profileSource = (hs && hs.profile_source) || "—";
    const running = hs && hs.host_running;
    const runningDot = running ? "sp-dot--ok" : "sp-dot--warn";
    const runningText = running ? (t("host-process-running") || "running") : (t("host-process-not-running") || "not running");
    const runningDetail = running
      ? ((hs && hs.host_processes && hs.host_processes.length ? hs.host_processes.join(", ") : (t("host-process-detected") || "detected")))
      : (t("host-process-detail") || "—");

    const prefs = (hs && hs.profile_keys) || {};
    const prefsText = Object.keys(prefs).length === 0
      ? (t("host-prefs-empty") || "(none)")
      : Object.entries(prefs).map(([k, v]) => `${k} = ${v}`).join("\n");

    const showJwtWarn = snap.cached_token && snap.cached_token.ttl_seconds < 600 && installed;
    const jwtWarnText = showJwtWarn ? (t("host-jwt-warn", { ttl: snap.cached_token.ttl_seconds }) || `JWT expires in ${snap.cached_token.ttl_seconds}s`) : "";

    const installDisabled = !host.last_generated_profile;

    return html`
      <article class="sp-host-card">
        <header class="sp-host-card__head">
          <h3 class="sp-host-card__name">${host.display_name || "—"}</h3>
          <span class="sp-badge ${badge.cls}">${badge.text}</span>
        </header>
        <table class="sp-status__board">
          <tbody>
            <tr>
              <th>Configuration profile</th>
              <td>
                <div class="sp-status__row">
                  <span class="sp-dot ${profileDot}" aria-hidden="true"></span>
                  <span>${profileText}</span>
                </div>
                <div class="sp-status__detail sp-u-mono ${profileSource === "—" ? "sp-u-muted" : ""}">${profileSource}</div>
              </td>
              <td class="sp-status__actions">
                <button class="sp-btn-primary" type="button" @click=${(e) => { e.stopPropagation(); this._generate(); }}>Generate</button>
                <button class="sp-btn-ghost" type="button" ?disabled=${installDisabled} title=${installDisabled ? "Generate first" : (host.last_generated_profile || "")} @click=${(e) => { e.stopPropagation(); this._install(); }}>Install</button>
                <button class="sp-btn-ghost" type="button" @click=${(e) => { e.stopPropagation(); this._reverify(); }}>Re-verify</button>
              </td>
            </tr>
            <tr>
              <th>Process</th>
              <td>
                <div class="sp-status__row">
                  <span class="sp-dot ${runningDot}" aria-hidden="true"></span>
                  <span>${runningText}</span>
                </div>
                <div class="sp-status__detail sp-u-mono ${running ? "" : "sp-u-muted"}">${runningDetail}</div>
              </td>
              <td class="sp-status__actions"></td>
            </tr>
          </tbody>
        </table>
        <details class="sp-status__prefs">
          <summary>Resolved profile keys</summary>
          <pre class="sp-log">${prefsText}</pre>
        </details>
        ${showJwtWarn ? html`<div class="sp-claude__warn">${jwtWarnText}</div>` : nothing}
      </article>
    `;
  }
}

customElements.define("sp-host-card", SpHostCard);
