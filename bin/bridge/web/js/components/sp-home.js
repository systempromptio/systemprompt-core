import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { hostStatus, isSetUp } from "/assets/js/utils/host-status.js";

/**
 * The pane the app opens on. It answers three questions in priority order and
 * nothing else: is traffic being governed, are my agents healthy, and is
 * anything waiting on me.
 *
 * The rule that makes it worth having: when everything is fine, Home is short
 * and boring. It grows only when something needs a person — so resist adding
 * anything here that is merely interesting. Usage and billing live on Account;
 * mechanism and diagnostics live on Status.
 */

// Worst first. A person opening this pane is looking for what is broken, and
// scanning past six healthy agents to find it is the whole problem with Status.
const STATE_RANK = { down: 0, attention: 1, unknown: 2, ok: 3 };

const TOKEN_EXPIRY_WARN_SECONDS = 600;

export class SpHome extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
    this.registerAction("jump", (trigger) => {
      const rail = document.querySelector("sp-rail");
      if (rail) { rail.activateTab(trigger.dataset.tab, { moveFocus: true }); }
    });
    this.registerAction("update-install", () => bridge.updateInstall());
    this.registerAction("update-restart", () => bridge.updateRestart());
    this.registerAction("sync", () => bridge.sync());
  }

  onConnect() {
    this.classList.add("sp-home");
    bridge.stateSnapshot()
      .then((s) => { this.snapshot = s; })
      .catch((e) => console.warn("home: snapshot failed", e));
    this.bridgeSubscribe("state.changed", (s) => { this.snapshot = s; });
  }

  _agents() {
    const hosts = ((this.snapshot && this.snapshot.host_apps) || []).filter(isSetUp);
    return hosts
      .map((host) => ({ host, status: hostStatus(host, this.snapshot) }))
      .sort((a, b) => {
        const rank = STATE_RANK[a.status.state] - STATE_RANK[b.status.state];
        if (rank !== 0) { return rank; }
        return (a.host.display_name || "").localeCompare(b.host.display_name || "");
      });
  }

  /**
   * The four state transitions that exist today and surface nowhere a user will
   * see them. Each returns one sentence and one button, or nothing at all.
   */
  _waiting() {
    const snap = this.snapshot;
    if (!snap) { return []; }
    const items = [];

    const update = snap.update || {};
    const version = update.version || "";
    if (update.phase === "ready") {
      items.push({
        id: "update-ready",
        tone: "ok",
        text: t("home-waiting-update-ready", { version })
          || `Version ${version} is downloaded and ready to install.`,
        action: { label: t("rail-profile-restart-cta") || "Restart to finish", act: "update-restart" },
      });
    } else if (update.phase === "available") {
      items.push({
        id: "update-available",
        tone: "ok",
        text: t("home-waiting-update-available", { version })
          || `Version ${version} is available.`,
        action: { label: t("rail-profile-update-cta") || "Download", act: "update-install" },
      });
    }

    const ttl = snap.cached_token && snap.cached_token.ttl_seconds;
    if (typeof ttl === "number" && ttl > 0 && ttl <= TOKEN_EXPIRY_WARN_SECONDS) {
      items.push({
        id: "session-expiring",
        tone: "warn",
        text: t("home-waiting-session", { minutes: Math.max(1, Math.round(ttl / 60)) })
          || `Your sign-in expires in ${Math.max(1, Math.round(ttl / 60))} minutes.`,
        action: null,
      });
    }

    // `needs_sign_in` is computed once, in the bridge, from the same predicate
    // that raises the desktop notification. Do not re-derive it from `state`
    // here: the two answers drifted apart and this card called four healthy
    // servers broken.
    const broken = ((snap.mcp_auth) || []).filter((s) => s.needs_sign_in);
    if (broken.length > 0) {
      items.push({
        id: "mcp-auth",
        tone: "warn",
        text: t("home-waiting-mcp", { count: broken.length, names: broken.map((s) => s.id).join(", ") })
          || `${broken.length} MCP server${broken.length === 1 ? "" : "s"} cannot authenticate: ${broken.map((s) => s.id).join(", ")}.`,
        action: { label: t("status-open-agents") || "Open Status", act: "jump", tab: "status" },
      });
    }

    if (snap.signed_in && !snap.last_sync_summary) {
      items.push({
        id: "never-synced",
        tone: "warn",
        text: t("home-waiting-never-synced") || "You have not synced yet, so no plugins are installed.",
        action: { label: t("sync-button") || "Sync now", act: "sync" },
      });
    }

    return items;
  }

  _renderAgents() {
    const rows = this._agents();
    if (rows.length === 0) {
      return `
        <section class="sp-home__section">
          <h2 data-l10n-id="home-agents-heading">Your agents</h2>
          <p class="sp-home__empty" data-l10n-id="agents-empty-title">No agents set up yet</p>
          <button class="sp-btn-ghost" type="button" data-action="jump" data-tab="agents"
            data-l10n-id="agents-action-add">Add an agent</button>
        </section>`;
    }
    const worst = rows[0].status.state;
    const body = rows.map(({ host, status }) => `
      <li class="sp-home__agent" data-state="${escapeHtml(status.state)}">
        <span class="sp-home__agent-name">${escapeHtml(host.display_name || host.id)}</span>
        <span class="sp-home__agent-reason">${escapeHtml(status.reason || status.label)}</span>
        <button class="sp-btn-ghost sp-home__agent-open" type="button"
          data-action="jump" data-tab="agents"
          aria-label="${escapeHtml(t("agent-open-details", { name: host.display_name || host.id }) || `Open details for ${host.display_name || host.id}`)}"
          >${escapeHtml(t("host-action-open") || "Open")}</button>
      </li>`).join("");
    return `
      <section class="sp-home__section" data-worst="${escapeHtml(worst)}">
        <h2 data-l10n-id="home-agents-heading">Your agents</h2>
        <ul class="sp-home__agents">${body}</ul>
      </section>`;
  }

  _renderWaiting() {
    const items = this._waiting();
    if (items.length === 0) { return ""; }
    const body = items.map((item) => `
      <li class="sp-home__waiting-item" data-tone="${escapeHtml(item.tone)}">
        <span>${escapeHtml(item.text)}</span>
        ${item.action
          ? `<button class="sp-btn" type="button" data-action="${escapeHtml(item.action.act)}"${item.action.tab ? ` data-tab="${escapeHtml(item.action.tab)}"` : ""}>${escapeHtml(item.action.label)}</button>`
          : ""}
      </li>`).join("");
    return `
      <section class="sp-home__section">
        <h2 data-l10n-id="home-waiting-heading">Waiting on you</h2>
        <ul class="sp-home__waiting">${body}</ul>
      </section>`;
  }

  render() {
    return `
      <header class="sp-tab__header">
        <h1 data-l10n-id="nav-home">Home</h1>
      </header>
      <sp-governance-strip variant="full"></sp-governance-strip>
      ${this._renderAgents()}
      ${this._renderWaiting()}
    `;
  }
}

reactive(SpHome.prototype, ["snapshot"]);
customElements.define("sp-home", SpHome);
