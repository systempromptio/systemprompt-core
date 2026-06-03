import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { fmtRelative } from "/assets/js/utils/format.js";

function chooseBadge(appInstalled, installed, partial, proxyState, modelsBlocked) {
  if (!appInstalled) { return { text: t("host-badge-app-missing") || "app not installed", cls: "sp-badge--err" }; }
  if (!installed && !partial) { return { text: t("host-badge-not-installed") || "profile not installed", cls: "sp-badge--warn" }; }
  if (partial) { return { text: t("host-badge-partial") || "partial", cls: "sp-badge--warn" }; }
  if (modelsBlocked) { return { text: t("host-badge-no-models") || "no compatible model", cls: "sp-badge--warn" }; }
  if (proxyState === "Unconfigured") { return { text: t("host-badge-awaiting") || "awaiting first launch", cls: "sp-badge--warn" }; }
  if (proxyState === "Listening") { return { text: t("host-badge-healthy") || "healthy", cls: "sp-badge--ok" }; }
  return { text: t("host-badge-proxy-down") || "proxy down", cls: "sp-badge--err" };
}

export class SpHostCard extends SpElement {
  constructor() {
    super();
    this.host = null;
    this.snapshot = null;
    this.registerAction("open", async () => {
      const installed = !!(this.host && this.host.snapshot && this.host.snapshot.app_installed);
      if (this.host && installed) {
        try { await bridge.agentOpen(this.host.id); } catch (e) { console.warn("open", e); }
      }
    });
    this.registerAction("download", async () => {
      const url = this.host && this.host.download_url;
      if (url) {
        try { await bridge.openExternalUrl(url); } catch (e) { console.warn("download", e); }
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
    const probing = !!host.probe_in_flight;
    const appInstalled = !!(hs && hs.app_installed);
    const modelsChecked = !!host.models_checked;
    const compatibleModels = Array.isArray(host.compatible_models) ? host.compatible_models : [];
    const unconfigured = Array.isArray(host.unconfigured_providers) ? host.unconfigured_providers : [];
    const modelsBlocked = modelsChecked && !host.compatible_models_available;
    const badge = hs
      ? chooseBadge(appInstalled, installed, partial, proxyState, modelsBlocked)
      : { text: "probing…", cls: "sp-badge--muted" };
    const spinnerMarkup = probing && hs ? `<span class="sp-spinner" aria-hidden="true"></span>` : "";

    let profileDot = "sp-dot--err";
    let profileText = t("host-profile-not-installed") || "not installed";
    if (installed) { profileDot = "sp-dot--ok"; profileText = t("host-profile-installed") || "installed"; }
    else if (partial) { profileDot = "sp-dot--warn"; profileText = t("host-profile-partial", { missing: missing.join(", ") }) || `partial (${missing.join(", ")})`; }

    const profileSource = (hs && hs.profile_source) || "—";
    const appDot = appInstalled ? "sp-dot--ok" : "sp-dot--err";
    const appInstalledText = appInstalled
      ? (t("host-app-installed") || "installed")
      : (t("host-app-not-installed") || "not installed");
    const downloadUrl = (host && host.download_url) || "";

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
    const jwtBlock = showJwtWarn ? `<div class="sp-claude__warn">${escapeHtml(jwtWarnText)}</div>` : "";

    const modelWarnText = modelsBlocked
      ? (unconfigured.length
          ? (t("host-models-unconfigured", { providers: unconfigured.join(", ") }) || `No usable model — provider(s) missing an API key: ${unconfigured.join(", ")}`)
          : (t("host-models-none") || "No compatible model is available for this host"))
      : "";
    const modelBlock = modelWarnText ? `<div class="sp-claude__warn">${escapeHtml(modelWarnText)}</div>` : "";

    const lastGen = host.last_generated_profile || null;

    const processLines = (hs && Array.isArray(hs.host_processes) && hs.host_processes.length)
      ? hs.host_processes.map((p) => escapeHtml(p)).join("<br>")
      : escapeHtml(runningDetail);

    const missingRow = partial && missing.length
      ? `<tr><th>${escapeHtml(t("host-missing-keys") || "Missing required keys")}</th><td><div class="sp-status__detail sp-u-mono">${escapeHtml(missing.join(", "))}</div></td></tr>`
      : "";

    const probedRow = hs && hs.probed_at_unix
      ? `<tr><th>${escapeHtml(t("host-last-probed") || "Last probed")}</th><td><div class="sp-status__detail">${escapeHtml(fmtRelative(hs.probed_at_unix))}</div></td></tr>`
      : "";

    const lastGenRow = lastGen
      ? `<tr><th>${escapeHtml(t("host-last-generated") || "Last generated")}</th><td><div class="sp-status__detail sp-u-mono">${escapeHtml(lastGen.path)}</div><div class="sp-status__detail sp-u-muted">${escapeHtml((lastGen.bytes / 1024).toFixed(1))} KB</div></td></tr>`
      : "";

    const profileUuidRow = lastGen && lastGen.profile_uuid
      ? `<tr><th>${escapeHtml(t("host-profile-uuid") || "Profile UUID")}</th><td><div class="sp-status__detail sp-u-mono">${escapeHtml(lastGen.profile_uuid)}</div></td></tr>`
      : "";

    const payloadUuidRow = lastGen && lastGen.payload_uuid
      ? `<tr><th>${escapeHtml(t("host-payload-uuid") || "Payload UUID")}</th><td><div class="sp-status__detail sp-u-mono">${escapeHtml(lastGen.payload_uuid)}</div></td></tr>`
      : "";

    const hostKindRow = host.kind
      ? `<tr><th>${escapeHtml(t("host-kind") || "Host kind")}</th><td><div class="sp-status__detail sp-u-mono">${escapeHtml(host.kind)}</div></td></tr>`
      : "";

    const configFormatRow = host.config_format
      ? `<tr><th>${escapeHtml(t("host-config-format") || "Config format")}</th><td><div class="sp-status__detail sp-u-mono">${escapeHtml(host.config_format)}</div></td></tr>`
      : "";

    const compatibleModelsRow = modelsChecked
      ? `<tr><th>${escapeHtml(t("host-compatible-models") || "Compatible models")}</th><td>${
          compatibleModels.length
            ? `<div class="sp-status__detail sp-u-mono">${escapeHtml(compatibleModels.join(", "))}</div>`
            : `<div class="sp-status__detail sp-u-muted">${escapeHtml(t("host-no-compatible-models") || "none available")}</div>`
        }</td></tr>`
      : "";

    const installLabelRow = host.install_action_label
      ? `<tr><th>${escapeHtml(t("host-install-label") || "Install action")}</th><td><div class="sp-status__detail">${escapeHtml(host.install_action_label)}</div></td></tr>`
      : "";

    const iconId = host.icon || host.id || "";
    const openLabel = escapeHtml(t("host-action-open") || "Open");
    const downloadLabel = escapeHtml(t("host-action-download") || "Download");
    const actionMarkup = appInstalled
      ? `<button class="sp-btn-ghost sp-host-card__open-btn" type="button" data-action="open">${openLabel}</button>`
      : (downloadUrl
          ? `<button class="sp-btn-ghost sp-host-card__open-btn" type="button" data-action="download" title="${escapeHtml(downloadUrl)}">${downloadLabel} ↗</button>`
          : `<button class="sp-btn-ghost sp-host-card__open-btn" type="button" data-action="open" disabled title="${escapeHtml(t("host-app-not-installed") || "not installed")}">${openLabel}</button>`);
    const logoTpl = document.getElementById(`tpl-host-logo-${iconId}`);
    const logoMarkup = logoTpl && logoTpl.content && logoTpl.content.firstElementChild
      ? logoTpl.content.firstElementChild.outerHTML.replace(/^<svg/, '<svg class="sp-host-card__logo"')
      : `<svg class="sp-host-card__logo" aria-hidden="true" viewBox="0 0 24 24"></svg>`;

    return `
      <article class="sp-host-card">
        <header class="sp-host-card__head">
          ${logoMarkup}
          <h3 class="sp-host-card__name">${escapeHtml(host.display_name || "—")}</h3>
          <span class="sp-badge ${badge.cls}">${escapeHtml(badge.text)}</span>
          ${spinnerMarkup}
          ${actionMarkup}
        </header>
        <table class="sp-status__board"><tbody>
          <tr>
            <th>Configuration profile</th>
            <td>
              <div class="sp-status__row"><span class="sp-dot ${profileDot}" aria-hidden="true"></span><span>${escapeHtml(profileText)}</span></div>
              <div class="sp-status__detail sp-u-mono ${profileSource === "—" ? "sp-u-muted" : ""}">${escapeHtml(profileSource)}</div>
            </td>
          </tr>
          <tr>
            <th>Application</th>
            <td>
              <div class="sp-status__row"><span class="sp-dot ${appDot}" aria-hidden="true"></span><span>${escapeHtml(appInstalledText)}</span></div>
            </td>
          </tr>
          <tr>
            <th>Process</th>
            <td>
              <div class="sp-status__row"><span class="sp-dot ${runningDot}" aria-hidden="true"></span><span>${escapeHtml(runningText)}</span></div>
              <div class="sp-status__detail sp-u-mono ${running ? "" : "sp-u-muted"}">${processLines}</div>
            </td>
          </tr>
          ${missingRow}
          ${probedRow}
          ${lastGenRow}
          ${profileUuidRow}
          ${payloadUuidRow}
          ${compatibleModelsRow}
          ${hostKindRow}
          ${configFormatRow}
          ${installLabelRow}
        </tbody></table>
        <details class="sp-status__prefs">
          <summary>Resolved profile keys</summary>
          <pre class="sp-log">${escapeHtml(prefsText)}</pre>
        </details>
        ${modelBlock}
        ${jwtBlock}
      </article>
    `;
  }
}

reactive(SpHostCard.prototype, ["host", "snapshot"]);
customElements.define("sp-host-card", SpHostCard);
