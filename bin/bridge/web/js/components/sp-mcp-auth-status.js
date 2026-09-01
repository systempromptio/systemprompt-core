import { SpElement, reactive } from "/assets/js/components/sp-element.js";
import { escapeHtml } from "/assets/js/utils/escape.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { publishSectionState } from "/assets/js/utils/format.js";
import { runAction } from "/assets/js/utils/action.js";
import { toneDot, toneSection } from "/assets/js/utils/verdict.js";

// The Status summary of MCP auth: one line per server, the bridge's verdict
// beside it. The full picture — tools, identity, URLs, per-server re-check —
// is the Marketplace's MCP detail; this pane only points there.
function row(srv) {
  const verdict = srv.verdict || { tone: "unknown", code: "unknown" };
  const tools = srv.shows_tools ? (t("mcp-tools", { count: (srv.tools || []).length }) || "") : "";
  return `
    <li class="sp-status__row sp-mcp-row" data-state="${escapeHtml(verdict.tone)}">
      <span class="sp-dot ${toneDot(verdict.tone)}" aria-hidden="true"></span>
      <span class="sp-mcp-row__name">${escapeHtml(srv.id || "")}</span>
      <span class="sp-mcp-row__label">${escapeHtml(t(`mcp-auth-${verdict.code}`) || "")}${tools ? ` · ${escapeHtml(tools)}` : ""}</span>
    </li>`;
}

export class SpMcpAuthStatus extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
    this.registerAction("recheck", (trigger) => runAction(trigger, {
      run: () => bridge.mcpAuthProbe(),
      success: () => t("mcp-rechecked") || "MCP servers re-checked.",
      context: t("mcp-recheck") || "Re-check",
    }));
    this.registerAction("open-marketplace", () => {
      const rail = document.querySelector("sp-rail");
      if (rail) { rail.activateTab("marketplace", { moveFocus: true }); }
    });
  }

  onConnect() {
    this.useSnapshot((s) => { this.snapshot = s; });
  }

  render() {
    const snap = this.snapshot || {};
    const servers = snap.mcp_auth || [];
    const probing = !!snap.mcp_auth_probe_in_flight;
    const recheckLabel = probing ? (t("mcp-checking") || "Checking…") : (t("mcp-recheck") || "Re-check");
    const body = servers.length
      ? `<ul class="sp-mcp-list">${servers.map(row).join("")}</ul>`
      : `<p class="sp-u-muted">${escapeHtml(t(`mcp-auth-${probing ? "unknown" : "no-servers"}`) || "")}</p>`;
    return `
      ${body}
      <div class="sp-kpi-card__foot">
        <span class="sp-kpi-card__foot-meta">${escapeHtml(t("mcp-live-roundtrip") || "")}</span>
        <button type="button" class="sp-btn-ghost" data-action="open-marketplace">${escapeHtml(t("mcp-open-marketplace") || "")}</button>
        <button type="button" class="sp-btn sp-btn--ghost" data-action="recheck" ${probing ? "disabled" : ""}>${escapeHtml(recheckLabel)}</button>
      </div>
    `;
  }

  afterRender() {
    const tone = (this.snapshot && this.snapshot.mcp_auth_tone) || "unknown";
    publishSectionState(this, tone, toneSection(tone));
  }
}

reactive(SpMcpAuthStatus.prototype, ["snapshot"]);
customElements.define("sp-mcp-auth-status", SpMcpAuthStatus);
