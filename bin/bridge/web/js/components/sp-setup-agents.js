import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { isInstalled } from "/assets/js/utils/verdict.js";
import { t } from "/assets/js/i18n.js";
import { announce } from "/assets/js/utils/announce.js";
import { repairHost } from "/assets/js/utils/host-actions.js";
import { notifyOk, notifyErr } from "/assets/js/utils/notify.js";

export class SpSetupAgents extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
    this.firstRun = null;
    this.registerAction("install-host", async (trigger) => {
      const id = trigger.dataset.hostId;
      if (!id) { return; }
      trigger.disabled = true;
      const name = trigger.dataset.hostName || id;
      try {
        const path = await repairHost(id);
        notifyOk(t("toast-agent-added", { name, path: path || "" })
          || `${name} added — wrote ${path || ""}. Restart ${name} to pick it up.`);
      } catch (e) {
        // The stage is the one thing the wizard knows that the drawer does not:
        // generating a profile and installing it fail for different reasons.
        trigger.dataset.failedStage = e.stage || "install";
        notifyErr(e, t(`setup-install-stage-${e.stage || "install"}`) || (e.stage || "install"));
        trigger.disabled = false;
      }
    });
  }

  onConnect() {
    this.classList.add("sp-setup-agent-list");
    this.useSnapshot((s) => {
      this.snapshot = s;
      // A late-mounting component would otherwise show nothing until the next
      // tick; the snapshot carries the run's current state.
      if (s && s.first_run) { this.firstRun = s.first_run; }
    });
    this.bridgeSubscribe("setup.progress", (p) => { this.firstRun = p; });
    this.bridgeSubscribe("host.changed", (host) => this._mergeHost(host));
  }

  _mergeHost(host) {
    if (!host || !host.id || !this.snapshot) { return; }
    const list = (this.snapshot.host_apps || []).slice();
    const idx = list.findIndex((h) => h.id === host.id);
    if (idx >= 0) { list[idx] = host; } else { list.push(host); }
    this.snapshot = { ...this.snapshot, host_apps: list };
  }

  _renderFirstRun() {
    const fr = this.firstRun;
    // One line for the run, not one live region around a list that repaints
    // every tick.
    const done = (fr.hosts || []).filter((h) => h.status === "done").length;
    const total = (fr.hosts || []).length;
    announce(t("setup-agents-progress", { done, total }) || `${done} of ${total} agents set up`);
    const glyphs = {
      pending: "·", probing: "…", generating: "…", installing: "…",
      done: "✓", failed: "✗", skipped: "–",
    };
    const rows = (fr.hosts || []).map((h) => {
      const failed = h.status === "failed";
      const detail = failed
        ? h.error || "failed"
        : (h.status === "skipped" ? "not detected on this device" : h.status);
      return `
        <div class="sp-setup-agent" data-state="${escapeHtml(h.status)}">
          <div class="sp-setup-agent__meta">
            <div class="sp-setup-agent__name">${escapeHtml(glyphs[h.status] || "·")} ${escapeHtml(h.display_name)}</div>
            <div class="sp-setup-agent__desc">${escapeHtml(detail)}</div>
          </div>
          ${failed ? `<button type="button" class="sp-btn-ghost" data-action="install-host" data-host-id="${escapeHtml(h.host_id)}" data-host-name="${escapeHtml(h.display_name)}">Retry</button>` : ""}
        </div>
      `;
    }).join("");
    const syncLabel = { pending: "Waiting", installing: "Syncing…", done: "Synced ✓", failed: "Sync failed ✗" }[fr.sync] || fr.sync;
    return `${rows}
      <div class="sp-setup-agent" data-state="${escapeHtml(fr.sync)}">
        <div class="sp-setup-agent__meta">
          <div class="sp-setup-agent__name">Plugins &amp; skills</div>
          <div class="sp-setup-agent__desc">${escapeHtml(syncLabel)}</div>
        </div>
      </div>`;
  }

  render() {
    // While first use is provisioning, the per-host status IS the list — the
    // manual install buttons would race the run.
    if (this.firstRun && this.firstRun.active) {
      return this._renderFirstRun();
    }
    const all = (this.snapshot && this.snapshot.host_apps) || [];
    // The last-sync manifest gates hosts: once any host is enabled, hide the
    // instance-disabled ones (host.changed merges can re-add them).
    const hosts = all.some((h) => h.enabled) ? all.filter((h) => h.enabled) : all;
    if (hosts.length === 0) {
      return `<div class="sp-u-muted">${escapeHtml(t("setup-agents-empty") || "No agents detected on this device.")}</div>`;
    }
    return hosts.map((host) => {
      const installed = isInstalled(host);
      const suffix = ` · ${t(`agent-kind-${host.kind}`) || ""}`;
      const cls = installed ? "sp-btn-ghost" : "sp-btn-primary";
      const label = installed
        ? (t("setup-agents-installed") || "Installed")
        : (t("setup-agents-install") || "Install profile");
      return `
        <div class="sp-setup-agent" data-state="${installed ? "installed" : "absent"}">
          <div class="sp-setup-agent__meta">
            <div class="sp-setup-agent__name">${escapeHtml(host.display_name + suffix)}</div>
            <div class="sp-setup-agent__desc">${escapeHtml(host.description || "")}</div>
          </div>
          <button type="button" class="${cls}" ${installed ? "disabled" : ""} data-action="install-host" data-host-id="${escapeHtml(host.id)}" data-host-name="${escapeHtml(host.display_name)}">${escapeHtml(label)}</button>
        </div>
      `;
    }).join("");
  }
}

reactive(SpSetupAgents.prototype, ["snapshot", "firstRun"]);
customElements.define("sp-setup-agents", SpSetupAgents);
