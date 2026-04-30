import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";

export class SpSetupAgents extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
    this.registerAction("install-host", async (trigger) => {
      const id = trigger.dataset.hostId;
      if (id) {
        try { await bridge.hostProfileGenerate(id); } catch (e) { console.warn("generate", e); }
      }
    });
  }

  onConnect() {
    this.classList.add("sp-setup-agent-list");
    this.setAttribute("aria-live", "polite");
    bridge.stateSnapshot().then((s) => { this.snapshot = s; }).catch((e) => console.warn("snapshot failed", e));
    this.bridgeSubscribe("state.changed", (s) => { this.snapshot = s; });
    this.bridgeSubscribe("host.changed", (host) => this._mergeHost(host));
  }

  _mergeHost(host) {
    if (!host || !host.id || !this.snapshot) { return; }
    const list = (this.snapshot.host_apps || []).slice();
    const idx = list.findIndex((h) => h.id === host.id);
    if (idx >= 0) { list[idx] = host; } else { list.push(host); }
    this.snapshot = { ...this.snapshot, host_apps: list };
  }

  render() {
    const hosts = (this.snapshot && this.snapshot.host_apps) || [];
    if (hosts.length === 0) {
      return `<div class="sp-u-muted">${escapeHtml(t("setup-agents-empty") || "No agents detected on this device.")}</div>`;
    }
    return hosts.map((host) => {
      const installed = host.snapshot?.profile_state?.kind === "installed";
      const suffix = host.kind === "cli_tool" ? " · CLI" : " · Desktop";
      const cls = installed ? "sp-btn-ghost" : "sp-btn-primary";
      const label = installed ? "Installed ✓" : "Install profile";
      return `
        <div class="sp-setup-agent" data-state="${installed ? "installed" : "absent"}">
          <div class="sp-setup-agent__meta">
            <div class="sp-setup-agent__name">${escapeHtml(host.display_name + suffix)}</div>
            <div class="sp-setup-agent__desc">${escapeHtml(host.description || "")}</div>
          </div>
          <button type="button" class="${cls}" ${installed ? "disabled" : ""} data-action="install-host" data-host-id="${escapeHtml(host.id)}">${escapeHtml(label)}</button>
        </div>
      `;
    }).join("");
  }
}

reactive(SpSetupAgents.prototype, ["snapshot"]);
customElements.define("sp-setup-agents", SpSetupAgents);
