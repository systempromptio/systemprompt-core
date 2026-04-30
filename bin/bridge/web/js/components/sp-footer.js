import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";

function hostState(snap) {
  if (snap.sync_in_flight) { return "running"; }
  if (snap.gateway_status && snap.gateway_status.state === "unreachable") { return "err"; }
  if (snap.signed_in) { return "ok"; }
  return "idle";
}

function dotClass(state) {
  if (state === "ok") { return "sp-dot--ok"; }
  if (state === "running") { return "sp-dot--probing"; }
  if (state === "err") { return "sp-dot--err"; }
  return "sp-dot--warn";
}

function docsUrl(platform) {
  if (platform === "macos") { return "https://systemprompt.io/documentation/services/cowork-deployment-macos"; }
  if (platform === "windows") { return "https://systemprompt.io/documentation/services/cowork-deployment-windows"; }
  return "https://systemprompt.io/documentation/services/cowork-deployment";
}

function isMissing(v) {
  return !v || v === "unknown" || v.startsWith("__");
}

function hasBuildMeta(sha, date) {
  return !isMissing(sha) && !isMissing(date);
}

export class SpFooter extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
    this.registerAction("open-external", (el, ev) => {
      const url = el && el.dataset && el.dataset.href;
      if (!url) { return; }
      if (ev && typeof ev.preventDefault === "function") { ev.preventDefault(); }
      bridge.openExternalUrl(url).catch((e) => console.warn("open url failed", url, e));
    });
  }

  onConnect() {
    this.classList.add("sp-footer");
    this.setAttribute("role", "contentinfo");
    bridge.stateSnapshot().then((s) => { this.snapshot = s; }).catch((e) => console.warn("snapshot failed", e));
    this.bridgeSubscribe("state.changed", (s) => { this.snapshot = s; });
  }

  render() {
    const snap = this.snapshot || {};
    const platformDisplay = this.dataset.platformDisplay || "";
    const platform = this.dataset.platform || "linux";
    const version = this.dataset.version || "";
    const gitSha = this.dataset.gitSha || "";
    const buildDate = this.dataset.buildDate || "";

    return `
      <div class="sp-footer__left">
        <span class="sp-footer__stat" title="Host status">
          <span class="sp-dot ${dotClass(hostState(snap))}" aria-hidden="true"></span>
          <span>${escapeHtml(platformDisplay)}</span>
        </span>
        <span class="sp-footer__sep" aria-hidden="true">·</span>
        <span class="sp-footer__path" title="Config path">${escapeHtml(snap.config_file || "—")}</span>
      </div>
      <div class="sp-footer__meta">
        ${hasBuildMeta(gitSha, buildDate)
          ? `<span class="sp-footer__version" title="Build ${escapeHtml(gitSha)} committed ${escapeHtml(buildDate)}">v${escapeHtml(version)} (${escapeHtml(gitSha)}, ${escapeHtml(buildDate)})</span>`
          : `<span class="sp-footer__version">v${escapeHtml(version)}</span>`}
        <span class="sp-footer__sep" aria-hidden="true">·</span>
        <a href="${escapeHtml(docsUrl(platform))}" data-href="${escapeHtml(docsUrl(platform))}" data-action="open-external" data-l10n-id="footer-docs">docs</a>
        <span class="sp-footer__sep" aria-hidden="true">·</span>
        <a href="https://systemprompt.io/documentation/licensing" data-href="https://systemprompt.io/documentation/licensing" data-action="open-external" data-l10n-id="footer-licensing">licensing</a>
      </div>
      <div class="sp-footer__right">
        <span class="sp-footer__hint"><span class="sp-kbd">⌘1</span><span class="sp-kbd">⌘2</span><span class="sp-kbd">⌘3</span><span class="sp-kbd">⌘4</span> <span data-l10n-id="footer-tabs-hint">tabs</span></span>
      </div>
    `;
  }
}

reactive(SpFooter.prototype, ["snapshot"]);
customElements.define("sp-footer", SpFooter);
