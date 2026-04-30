import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";

function fmtCount(n) {
  const v = Number(n) || 0;
  if (v >= 1_000_000) { return `${(v / 1_000_000).toFixed(1)}M`; }
  if (v >= 1_000) { return `${(v / 1_000).toFixed(1)}k`; }
  return String(v);
}

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

export class SpFooter extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
  }

  onConnect() {
    this.classList.add("sp-footer");
    this.setAttribute("role", "contentinfo");
    bridge.stateSnapshot().then((s) => { this.snapshot = s; }).catch(() => {});
    this.bridgeSubscribe("state.changed", (s) => { this.snapshot = s; });
    this.bridgeSubscribe("proxy.stats", () => {
      bridge.stateSnapshot().then((s) => { this.snapshot = s; }).catch(() => {});
    });
  }

  render() {
    const snap = this.snapshot || {};
    const stats = snap.proxy_stats || {};
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
        <span class="sp-footer__version" title="Build ${escapeHtml(gitSha)} committed ${escapeHtml(buildDate)}">v${escapeHtml(version)} (${escapeHtml(gitSha)}, ${escapeHtml(buildDate)})</span>
        <span class="sp-footer__sep" aria-hidden="true">·</span>
        <a href="https://systemprompt.io/docs/bridge/${escapeHtml(platform)}" target="_blank" rel="noopener noreferrer" data-l10n-id="footer-docs">docs</a>
        <span class="sp-footer__sep" aria-hidden="true">·</span>
        <a href="mailto:ed@systemprompt.io" data-l10n-id="footer-licensing">licensing</a>
      </div>
      <div class="sp-footer__right">
        <span class="sp-footer__hint"><span class="sp-kbd">⌘1</span><span class="sp-kbd">⌘2</span><span class="sp-kbd">⌘3</span><span class="sp-kbd">⌘4</span> <span data-l10n-id="footer-tabs-hint">tabs</span></span>
      </div>
      <span hidden data-role="lane-msgs">${escapeHtml(fmtCount(stats.messages_total))}</span>
      <span hidden data-role="lane-tin">${escapeHtml(fmtCount(stats.tokens_in_total))}</span>
      <span hidden data-role="lane-tout">${escapeHtml(fmtCount(stats.tokens_out_total))}</span>
    `;
  }
}

reactive(SpFooter.prototype, ["snapshot"]);
customElements.define("sp-footer", SpFooter);
