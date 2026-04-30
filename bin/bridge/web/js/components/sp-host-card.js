import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";

function chooseBadge(installed, partial, proxyState) {
  if (!installed) { return { text: t("host-badge-not-installed") || "not installed", cls: "sp-badge--warn" }; }
  if (partial) { return { text: t("host-badge-partial") || "partial", cls: "sp-badge--warn" }; }
  if (proxyState === "Unconfigured") { return { text: t("host-badge-awaiting") || "awaiting first launch", cls: "sp-badge--warn" }; }
  if (proxyState === "Listening") { return { text: t("host-badge-healthy") || "healthy", cls: "sp-badge--ok" }; }
  return { text: t("host-badge-proxy-down") || "proxy down", cls: "sp-badge--err" };
}

export class SpHostCard extends SpElement {
  constructor() {
    super();
    this.host = null;
    this.snapshot = null;
    this.registerAction("generate", async () => {
      if (this.host) {
        try { await bridge.hostProfileGenerate(this.host.id); } catch (e) { console.warn("generate", e); }
      }
    });
    this.registerAction("install", async () => {
      if (this.host && this.host.last_generated_profile) {
        try { await bridge.hostProfileInstall(this.host.id, this.host.last_generated_profile); } catch (e) { console.warn("install", e); }
      }
    });
    this.registerAction("reverify", async () => {
      if (this.host) {
        try { await bridge.hostProbe(this.host.id); } catch (e) { console.warn("reverify", e); }
      }
    });
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
    const installTitle = installDisabled ? "Generate first" : (host.last_generated_profile || "");
    const jwtBlock = showJwtWarn ? `<div class="sp-claude__warn">${escapeHtml(jwtWarnText)}</div>` : "";

    return `
      <article class="sp-host-card">
        <header class="sp-host-card__head">
          <h3 class="sp-host-card__name">${escapeHtml(host.display_name || "—")}</h3>
          <span class="sp-badge ${badge.cls}">${escapeHtml(badge.text)}</span>
        </header>
        <table class="sp-status__board"><tbody>
          <tr>
            <th>Configuration profile</th>
            <td>
              <div class="sp-status__row"><span class="sp-dot ${profileDot}" aria-hidden="true"></span><span>${escapeHtml(profileText)}</span></div>
              <div class="sp-status__detail sp-u-mono ${profileSource === "—" ? "sp-u-muted" : ""}">${escapeHtml(profileSource)}</div>
            </td>
            <td class="sp-status__actions">
              <button class="sp-btn-primary" type="button" data-action="generate">Generate</button>
              <button class="sp-btn-ghost" type="button" ${installDisabled ? "disabled" : ""} title="${escapeHtml(installTitle)}" data-action="install">Install</button>
              <button class="sp-btn-ghost" type="button" data-action="reverify">Re-verify</button>
            </td>
          </tr>
          <tr>
            <th>Process</th>
            <td>
              <div class="sp-status__row"><span class="sp-dot ${runningDot}" aria-hidden="true"></span><span>${escapeHtml(runningText)}</span></div>
              <div class="sp-status__detail sp-u-mono ${running ? "" : "sp-u-muted"}">${escapeHtml(runningDetail)}</div>
            </td>
            <td class="sp-status__actions"></td>
          </tr>
        </tbody></table>
        <details class="sp-status__prefs">
          <summary>Resolved profile keys</summary>
          <pre class="sp-log">${escapeHtml(prefsText)}</pre>
        </details>
        ${jwtBlock}
      </article>
    `;
  }
}

reactive(SpHostCard.prototype, ["host", "snapshot"]);
customElements.define("sp-host-card", SpHostCard);
