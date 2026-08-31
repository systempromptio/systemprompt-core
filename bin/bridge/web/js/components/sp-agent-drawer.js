import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import {
  hostStatus, badgeSuffix, isSetUp,
  APP_INSTALLED, APP_NOT_INSTALLED, appInstallState,
} from "/assets/js/utils/host-status.js";
import { repairHost, runHostAction, openHostConfig } from "/assets/js/utils/host-actions.js";
import { notifyOk, notifyErr, notifyAction } from "/assets/js/utils/notify.js";
import { hostLogoMarkup } from "/assets/js/components/sp-agent-row.js";
import { trapTab, setBackgroundInert } from "/assets/js/utils/focus-trap.js";
import { fmtDurationLong } from "/assets/js/utils/format.js";

/**
 * The right-hand slide-over that holds everything the Agents list deliberately
 * does not show. Two modes on one surface:
 *
 *   "detail" — one agent: its health, its models, and (collapsed) the technical
 *              configuration that used to be the entire page.
 *   "add"    — the picker behind the header's "+ Add agent".
 *
 * The app has no <dialog> and no modal anywhere, so the dialog semantics here
 * are hand-rolled: role/aria-modal, Escape to close, focus moved in on open and
 * returned to the opener on close.
 */

// These are `ApiSurface` tags, which is what `host.model_protocols` carries and
// what `hostModelFilterSet` parses back. They are NOT `ProviderProtocol` tags:
// this list once held "openai-chat"/"openai-responses", which matched nothing
// coming from the bridge (the surface tag is plain "openai"), so the OpenAI
// boxes rendered unchecked while OpenAI was on, and ticking one saved a filter
// the bridge then dropped as unparseable.
//
// Only the advertised surfaces get a checkbox. `backend` is never advertised to
// a client, so there is nothing for a user to decide about it -- but it can
// still be in effect, and `_captureFilter` carries it through untouched rather
// than silently dropping it on save.
const WIRE_SURFACES = ["anthropic", "openai", "gemini"];

// The wire name is what the filter actually stores, so it stays on screen -- but
// as the secondary line. A checkbox whose whole accessible name is "openai"
// tells a user nothing about what they are turning off.
const SURFACE_L10N = {
  "anthropic": "proto-anthropic",
  "openai": "proto-openai",
  "gemini": "proto-gemini",
};

const SURFACE_LABEL = {
  "anthropic": "Claude models",
  "openai": "OpenAI models",
  "gemini": "Gemini models",
};

function row(label, body) {
  return `<tr><th>${escapeHtml(label)}</th><td>${body}</td></tr>`;
}

function hostName(host) {
  return (host && (host.display_name || host.id)) || "this agent";
}

function actionLabel(kind) {
  return t(`agent-action-${kind}`) || kind;
}

// Repair and Add both rewrite a config file the user cannot see, and both need
// the agent restarted afterwards. Saying which file was written is the whole
// difference between "something happened" and a report.
function successLine(kind, host, result) {
  switch (kind) {
    case "repair":
    case "add":    return t("toast-agent-repaired", { name: hostName(host), path: result || "" })
      || `${hostName(host)} re-configured — wrote ${result || ""}. Restart ${hostName(host)} to pick it up.`;
    case "verify": return t("toast-agent-verified", { name: hostName(host) })
      || `${hostName(host)} re-checked.`;
    default:       return "";
  }
}

function detailText(value, { mono = false, muted = false } = {}) {
  const cls = ["sp-status__detail", mono ? "sp-u-mono" : "", muted ? "sp-u-muted" : ""]
    .filter(Boolean).join(" ");
  return `<div class="${cls}">${escapeHtml(value)}</div>`;
}

export class SpAgentDrawer extends SpElement {
  constructor() {
    super();
    this.mode = null;      // null | "detail" | "add"
    this.hostId = null;
    this.hosts = [];
    this.snapshot = null;
    this.gated = false;
    this.busyId = null;
    this.filterDraft = null;
    this.confirmRemove = false;
    this._returnFocusTo = null;
    this._needsFocus = false;

    this.registerAction("close", () => this.close());
    this.registerAction("scrim", () => this.close());

    this.registerAction("act", async (trigger) => {
      const host = this._hostById(trigger.dataset.hostId) || this._host();
      const kind = trigger.dataset.kind;
      if (!host || !kind || this.busyId) { return; }
      this.busyId = host.id;
      try {
        const result = await runHostAction(kind, host);
        const line = successLine(kind, host, result);
        if (line) { notifyOk(line); }
      } catch (e) {
        notifyErr(e, actionLabel(kind));
      } finally {
        this.busyId = null;
      }
    });

    this.registerAction("open-config", async () => {
      const host = this._host();
      if (!host) { return; }
      try { await openHostConfig(host.id); }
      catch (e) { notifyErr(e, t("agent-action-open-config") || "Show config file"); }
    });

    this.registerAction("add-host", async (trigger) => {
      const id = trigger.dataset.hostId;
      if (!id || this.busyId) { return; }
      this.busyId = id;
      const host = this._hostById(id);
      try {
        const path = await repairHost(id);
        notifyOk(t("toast-agent-added", { name: hostName(host), path })
          || `${hostName(host)} added — wrote ${path}. Restart ${hostName(host)} to pick it up.`);
      } catch (e) {
        notifyErr(e, t("agent-action-add") || "Add");
      } finally {
        this.busyId = null;
      }
    });

    this.registerAction("change:proto", () => this._captureFilter());
    this.registerAction("change:model-all", () => this._captureFilter());

    this.registerAction("saveModelFilter", async (trigger) => {
      const host = this._host();
      if (!host || !this.filterDraft) { return; }
      const protocols = this.filterDraft.all ? [] : this.filterDraft.protocols;
      trigger.disabled = true;
      try {
        await bridge.hostModelFilterSet(host.id, protocols);
        this.filterDraft = null;
        notifyOk(t("toast-model-filter-saved") || "Model filter saved to your systemprompt account.");
      } catch (e) {
        notifyErr(e, t("host-model-filter-save") || "Save filter");
      } finally {
        if (trigger.isConnected) { trigger.disabled = false; }
      }
    });

    this.registerAction("resetModelFilter", async (trigger) => {
      const host = this._host();
      if (!host) { return; }
      trigger.disabled = true;
      try {
        await bridge.hostModelFilterSet(host.id, null);
        this.filterDraft = null;
        notifyOk(t("toast-model-filter-reset") || "Model filter reset to this agent's default.");
      } catch (e) {
        notifyErr(e, t("host-model-filter-reset") || "Reset to default");
      } finally {
        if (trigger.isConnected) { trigger.disabled = false; }
      }
    });

    this.registerAction("remove-agent", () => { this.confirmRemove = true; });
    this.registerAction("cancel-remove", () => { this.confirmRemove = false; });

    this.registerAction("confirm-remove", async (trigger) => {
      const host = this._host();
      if (!host || this.busyId) { return; }
      this.busyId = host.id;
      try {
        const result = await bridge.agentUninstall(host.id);
        this.confirmRemove = false;
        // The reply distinguishes a removal from an instruction, because on
        // macOS the profile is held by the OS and only the user can withdraw it.
        if (result && result.removed) {
          notifyOk(t("toast-agent-removed", { name: hostName(host) })
            || `${hostName(host)} removed. Restart it to drop the old settings.`);
          this.close();
        } else {
          notifyAction(result && result.instruction
            ? t("toast-agent-remove-manual", { name: hostName(host), instruction: result.instruction })
              || `${hostName(host)}: ${result.instruction}`
            : t("toast-agent-remove-nothing", { name: hostName(host) })
              || `${hostName(host)} had nothing left to remove.`);
        }
      } catch (e) {
        notifyErr(e, t("agent-action-remove") || "Remove agent");
      } finally {
        this.busyId = null;
        if (trigger.isConnected) { trigger.disabled = false; }
      }
    });
  }

  _captureFilter() {
    const allEl = this.querySelector("[data-model-all]");
    const host = this._host();
    const saved = (host && Array.isArray(host.model_protocols)) ? host.model_protocols : [];
    // Surfaces with no checkbox are not the user's to change here, so they
    // survive the save rather than being deleted by omission.
    const unshown = saved.filter((tag) => !WIRE_SURFACES.includes(tag));
    this.filterDraft = {
      all: allEl ? allEl.checked : false,
      protocols: Array.from(this.querySelectorAll("[data-proto]"))
        .filter((el) => el.checked)
        .map((el) => el.dataset.proto)
        .concat(unshown),
    };
  }

  onConnect() {
    this._onKeydown = (e) => {
      if (!this.mode) { return; }
      if (e.key === "Escape") { e.stopPropagation(); this.close(); return; }
      // aria-modal="true" claims the rest of the page is unavailable. Without
      // this, Tab walked straight out of the dialog into the rail and the
      // activity log, both sitting behind the scrim.
      trapTab(e, this.querySelector(".sp-drawer__panel"));
    };
    document.addEventListener("keydown", this._onKeydown, true);
    this._unsubs.push(() => document.removeEventListener("keydown", this._onKeydown, true));
  }

  /** @param {"detail"|"add"} mode */
  open(mode, hostId, opener) {
    this._returnFocusTo = opener || document.activeElement;
    this._needsFocus = true;
    this.hostId = hostId || null;
    this.filterDraft = null;
    this.confirmRemove = false;
    this.mode = mode;
    document.body.classList.add("is-drawer-open");
    setBackgroundInert(true, this);
  }

  close() {
    if (!this.mode) { return; }
    this.mode = null;
    this.hostId = null;
    this.filterDraft = null;
    this.confirmRemove = false;
    document.body.classList.remove("is-drawer-open");
    setBackgroundInert(false, this);
    const target = this._returnFocusTo;
    this._returnFocusTo = null;
    if (target && typeof target.focus === "function" && target.isConnected) { target.focus(); }
  }

  _hostById(id) {
    return id ? (this.hosts || []).find((h) => h.id === id) || null : null;
  }

  _host() {
    return this._hostById(this.hostId);
  }

  render() {
    if (!this.mode) { return `<div class="sp-drawer" hidden></div>`; }
    const body = this.mode === "add" ? this._renderAdd() : this._renderDetail();
    return `
      <div class="sp-drawer" data-open="true">
        <div class="sp-drawer__scrim" data-action="scrim" aria-hidden="true"></div>
        <section class="sp-drawer__panel" role="dialog" aria-modal="true"
                 aria-label="${escapeHtml(body.title)}">
          <header class="sp-drawer__head">
            ${body.head}
            <button class="sp-drawer__close" type="button" data-action="close"
                    aria-label="${escapeHtml(t("drawer-close") || "Close")}">✕</button>
          </header>
          <div class="sp-drawer__body">${body.content}</div>
        </section>
      </div>
    `;
  }

  // --- add mode --------------------------------------------------------------

  _renderAdd() {
    const title = t("agents-add-heading") || "Add an agent";
    const hosts = this.hosts || [];
    const gateNote = this.gated
      ? ""
      : `<p class="sp-drawer__note sp-u-muted">${escapeHtml(
          t("agents-add-provisional")
            || "This list is provisional until this computer has synced with systemprompt."
        )}</p>`;

    const items = hosts.length === 0
      ? `<p class="sp-u-muted">${escapeHtml(t("agents-add-empty") || "No agents are available for this installation.")}</p>`
      : hosts.map((host) => this._renderAddItem(host)).join("");

    return {
      title,
      head: `<h2 class="sp-drawer__title">${escapeHtml(title)}</h2>`,
      content: `
        <p class="sp-drawer__lede">${escapeHtml(
          t("agents-add-lede")
            || "Pick a coding agent to route through systemprompt. Adding one writes its configuration profile — you do not need to configure anything by hand."
        )}</p>
        ${gateNote}
        <div class="sp-drawer__list">${items}</div>
      `,
    };
  }

  _renderAddItem(host) {
    const added = isSetUp(host);
    const appState = appInstallState(host);
    const busy = this.busyId === host.id;
    const suffix = host.kind === "cli_tool"
      ? (t("agent-kind-cli") || "Command line")
      : (t("agent-kind-desktop") || "Desktop app");

    let action;
    if (added) {
      action = `<span class="sp-badge sp-badge--ok">${escapeHtml(t("agents-add-added") || "Added")}</span>`;
    } else if (appState === APP_NOT_INSTALLED && host.download_url) {
      action = `<button class="sp-btn-ghost" type="button" data-action="act"
                        data-kind="download" data-host-id="${escapeHtml(host.id)}"
                        title="${escapeHtml(host.download_url)}">${escapeHtml(t("host-action-download") || "Download")} ↗</button>`;
    } else {
      action = `<button class="sp-btn-primary" type="button" data-action="add-host"
                        data-host-id="${escapeHtml(host.id)}" ${busy ? "disabled" : ""}>${escapeHtml(
                          busy ? (t("agent-action-working") || "Working…" || "Working…") : (t("agent-action-add") || "Add" || "Add")
                        )}</button>`;
    }

    const note = !added && appState === APP_NOT_INSTALLED
      ? (t("agent-reason-app-missing") || "The app is not installed on this computer")
      : (host.description || "");

    return `
      <div class="sp-drawer__item" data-key="${escapeHtml(host.id)}" data-added="${added}">
        ${hostLogoMarkup(host.icon || host.id || "", "sp-drawer__item-logo")}
        <div class="sp-drawer__item-meta">
          <div class="sp-drawer__item-name">${escapeHtml(host.display_name || host.id)}</div>
          <div class="sp-drawer__item-desc">${escapeHtml(suffix)}${note ? ` · ${escapeHtml(note)}` : ""}</div>
        </div>
        ${action}
      </div>
    `;
  }

  // --- detail mode -----------------------------------------------------------

  _renderDetail() {
    const host = this._host();
    if (!host) {
      // An empty head renders a panel with no title and no close button, so
      // Escape and the scrim are the only ways out of it.
      const gone = t("agents-detail-gone") || "Agent not available";
      return {
        title: gone,
        head: `<div class="sp-drawer__headmeta"><h2 class="sp-drawer__title">${escapeHtml(gone)}</h2></div>`,
        content: `<p class="sp-u-muted">${escapeHtml(t("agents-detail-gone-body") || "This agent is no longer available on this computer.")}</p>`,
      };
    }
    const name = host.display_name || host.id;
    const status = hostStatus(host, this.snapshot);
    const hs = host.snapshot || null;

    return {
      title: name,
      head: `
        ${hostLogoMarkup(host.icon || host.id || "", "sp-drawer__logo")}
        <div class="sp-drawer__headmeta">
          <h2 class="sp-drawer__title">${escapeHtml(name)}</h2>
          <span class="sp-drawer__status">
            <span class="sp-badge sp-badge--${escapeHtml(badgeSuffix(status.state))}">${escapeHtml(status.label)}</span>
          </span>
        </div>
      `,
      content: `
        <p class="sp-drawer__lede">${escapeHtml(status.reason)}</p>
        ${this._actions(host, status)}
        ${this._warnings(host, status)}
        ${this._healthSection(host, hs)}
        ${this._modelsSection(host)}
        ${this._configSection(host, hs)}
        ${this._removeSection(host, hs)}
      `,
    };
  }

  // Adding an agent was one click and removing one was impossible. The confirm
  // is inline rather than a modal because the app has no modal, and it names the
  // file that is about to lose its keys.
  _removeSection(host, hs) {
    if (!isSetUp(host)) { return ""; }
    const path = (hs && hs.profile_source) || "";
    const body = this.confirmRemove
      ? `
        <p>${escapeHtml(path
            ? t("agent-remove-confirm-path", { name: hostName(host), path })
        || `Remove ${hostName(host)} from systemprompt? This strips its systemprompt keys from ${path}.`
            : t("agent-remove-confirm", { name: hostName(host) })
        || `Remove ${hostName(host)} from systemprompt?`)}</p>
        <div class="sp-drawer__filter-actions">
          <button class="sp-btn-danger" type="button" data-action="confirm-remove" ${this.busyId === host.id ? "disabled" : ""}>${escapeHtml(
            this.busyId === host.id ? t("agent-action-working") || "Working…" : t("agent-remove-confirm-button") || "Remove it")}</button>
          <button class="sp-btn-ghost" type="button" data-action="cancel-remove">${escapeHtml(t("agent-remove-cancel") || "Keep it")}</button>
        </div>`
      : `
        <p class="sp-u-muted">${escapeHtml(t("agent-remove-explainer") || "Removing takes this agent's settings back out of its configuration file. It does not uninstall the app.")}</p>
        <div class="sp-drawer__filter-actions">
          <button class="sp-btn-ghost sp-btn-ghost--danger" type="button" data-action="remove-agent">${escapeHtml(t("agent-action-remove") || "Remove agent")}</button>
        </div>`;
    return `<section class="sp-drawer__section sp-drawer__section--danger">
      <h3 class="sp-drawer__section-title">${escapeHtml(t("agent-section-remove") || "Remove")}</h3>
      ${body}
    </section>`;
  }

  _actions(host, status) {
    const busy = this.busyId === host.id;
    const appState = appInstallState(host);
    const buttons = [];
    const primaryKind = status.action && status.action.kind;

    // The recommended action leads and is the only primary button; the rest stay
    // available but visually secondary, so there is never a question of which
    // one to press.
    const needsFixing = status.state !== "ok";
    if (status.action) {
      buttons.push(`<button class="${needsFixing ? "sp-btn-primary" : "sp-btn-ghost"}" type="button" data-action="act"
        data-kind="${escapeHtml(primaryKind)}" ${busy ? "disabled" : ""}>${escapeHtml(
          busy ? (t("agent-action-working") || "Working…" || "Working…") : status.action.label
        )}${primaryKind === "download" ? " ↗" : ""}</button>`);
    }
    if (primaryKind !== "open" && appState !== APP_NOT_INSTALLED) {
      buttons.push(`<button class="sp-btn-ghost" type="button" data-action="act" data-kind="open">${escapeHtml(t("host-action-open") || "Open")}</button>`);
    }
    if (primaryKind !== "repair" && primaryKind !== "add") {
      buttons.push(`<button class="sp-btn-ghost" type="button" data-action="act" data-kind="repair" ${busy ? "disabled" : ""}>${escapeHtml(t("agent-action-repair") || "Repair")}</button>`);
    }
    if (primaryKind !== "verify") {
      buttons.push(`<button class="sp-btn-ghost" type="button" data-action="act" data-kind="verify">${escapeHtml(t("agent-action-verify") || "Verify" || "Verify")}</button>`);
    }
    buttons.push(`<button class="sp-btn-ghost" type="button" data-action="open-config">${escapeHtml(t("agent-action-open-config") || "Show config file" || "Show config file")}</button>`);
    return `<div class="sp-drawer__actions">${buttons.join("")}</div>`;
  }

  _warnings(host, status) {
    const out = [];
    const snap = this.snapshot || {};
    if (status.state === "attention" && status.action && status.action.kind === "repair") {
      out.push(t("agent-repair-explainer")
        || "Repair rewrites this agent's configuration profile and re-applies it. Restart the agent afterwards.");
    }
    if (snap.cached_token && snap.cached_token.ttl_seconds < 600 && isSetUp(host)) {
      const ttl = fmtDurationLong(snap.cached_token.ttl_seconds);
      out.push(t("host-jwt-warn", { ttl })
        || `This agent's session expires in ${ttl}. Repair the agent to renew it.`);
    }
    return out.map((w) => `<div class="sp-claude__warn">${escapeHtml(w)}</div>`).join("");
  }

  _healthSection(host, hs) {
    if (!hs) {
      return this._section("agent-section-health", "Health",
        `<p class="sp-u-muted">${escapeHtml(t("agent-state-checking") || "Checking…")}</p>`);
    }
    const profileState = hs.profile_state || { kind: "absent" };
    const missing = profileState.missing_required || [];
    const kind = profileState.kind;

    let profileDot = "sp-dot--err";
    let profileText = t("host-profile-not-installed") || "not installed";
    if (kind === "installed") { profileDot = "sp-dot--ok"; profileText = t("host-profile-installed") || "installed"; }
    else if (kind === "partial") { profileDot = "sp-dot--warn"; profileText = t("host-profile-partial", { missing: missing.join(", ") }) || `partial (${missing.join(", ")})`; }
    else if (kind === "stale") { profileDot = "sp-dot--warn"; profileText = t("host-profile-stale") || "secret out of date — re-apply profile"; }

    const appState = appInstallState(host);
    const appDot = appState === APP_INSTALLED ? "sp-dot--ok" : (appState === APP_NOT_INSTALLED ? "sp-dot--err" : "sp-dot--warn");
    const appText = appState === APP_INSTALLED
      ? (t("host-app-installed") || "installed")
      : (appState === APP_NOT_INSTALLED ? (t("host-app-not-installed") || "not installed") : (t("host-app-unknown") || "could not determine"));

    const running = !!hs.host_running;
    const processes = Array.isArray(hs.host_processes) ? hs.host_processes : [];
    const runningText = running ? (t("host-process-running") || "running") : (t("host-process-not-running") || "not running");

    const rows = [
      row(t("agent-row-profile") || "Configuration profile",
        `<div class="sp-status__row"><span class="sp-dot ${profileDot}" aria-hidden="true"></span><span>${escapeHtml(profileText)}</span></div>`),
      row(t("agent-row-app") || "Application",
        `<div class="sp-status__row"><span class="sp-dot ${appDot}" aria-hidden="true"></span><span>${escapeHtml(appText)}</span></div>`),
      row(t("agent-row-process") || "Process",
        `<div class="sp-status__row"><span class="sp-dot ${running ? "sp-dot--ok" : "sp-dot--warn"}" aria-hidden="true"></span><span>${escapeHtml(runningText)}</span></div>`
        + (processes.length ? detailText(processes.join(", "), { mono: true }) : "")),
    ];
    if (kind === "partial" && missing.length) {
      rows.push(row(t("host-missing-keys") || "Missing required keys", detailText(missing.join(", "), { mono: true })));
    }
    return this._section("agent-section-health", "Health", `<table class="sp-status__board"><tbody>${rows.join("")}</tbody></table>`);
  }

  _modelsSection(host) {
    // "Not checked yet" is a third state, and rendering nothing for it made it
    // indistinguishable from an agent with no models at all.
    if (!host.models_checked) {
      return this._section("agent-section-models", "Models", `
        <p class="sp-u-muted">${escapeHtml(t("agent-models-unchecked") || "This agent's models have not been checked on this computer yet.")}</p>
        <div class="sp-drawer__filter-actions">
          <button class="sp-btn-ghost" type="button" data-action="act" data-kind="verify">${escapeHtml(t("agent-action-verify") || "Verify")}</button>
        </div>
      `);
    }
    const compatible = Array.isArray(host.compatible_models) ? host.compatible_models : [];
    const saved = Array.isArray(host.model_protocols) ? host.model_protocols : [];
    const draft = this.filterDraft;
    const effective = draft ? draft.protocols : saved;
    const allModels = draft ? draft.all : saved.length === 0;
    const overridden = !!host.model_protocols_overridden;

    const checks = WIRE_SURFACES.map((p) =>
      `<label class="sp-drawer__proto"><input type="checkbox" data-change="proto" data-proto="${escapeHtml(p)}" ${effective.includes(p) ? "checked" : ""}> <span class="sp-drawer__proto-name">${escapeHtml(t(SURFACE_L10N[p]) || SURFACE_LABEL[p])}</span> <span class="sp-u-mono sp-u-muted">${escapeHtml(p)}</span></label>`
    ).join("");

    const modelsBody = compatible.length
      ? `<details class="sp-status__prefs"><summary>${escapeHtml(t("agent-models-count", { count: compatible.length }) || `${compatible.length} models available`)}</summary><div class="sp-status__detail sp-u-mono">${escapeHtml(compatible.join(", "))}</div></details>`
      : `<div class="sp-status__detail sp-u-muted">${escapeHtml(t("host-no-compatible-models") || "none available")}</div>`;

    const rows = [
      row(t("host-compatible-models") || "Compatible models", modelsBody),
      row(t("host-model-filter") || "Model filter", `
        <label class="sp-drawer__proto"><input type="checkbox" data-change="model-all" data-model-all ${allModels ? "checked" : ""}> <span>${escapeHtml(t("host-model-filter-all") || "All models")}</span></label>
        <div class="sp-drawer__protos">${checks}</div>
        ${detailText(overridden ? (t("host-model-filter-custom") || "custom override") : (t("host-model-filter-default") || "host default"), { muted: true })}
        ${detailText(t("agent-model-filter-hint") || "Saved to your systemprompt account — you must be signed in.", { muted: true })}
        ${draft ? `<div class="sp-drawer__dirty">${escapeHtml(t("host-model-filter-unsaved") || "Unsaved changes.")}</div>` : ""}
        <div class="sp-drawer__filter-actions">
          <button class="sp-btn-primary" type="button" data-action="saveModelFilter" ${draft ? "" : "disabled"}>${escapeHtml(t("host-model-filter-save") || "Save filter" || "Save filter")}</button>
          <button class="sp-btn-ghost" type="button" data-action="resetModelFilter">${escapeHtml(t("host-model-filter-reset") || "Reset to default" || "Reset to default")}</button>
        </div>`),
    ];
    return this._section("agent-section-models", "Models", `<table class="sp-status__board"><tbody>${rows.join("")}</tbody></table>`);
  }

  _configSection(host, hs) {
    const lastGen = host.last_generated_profile || null;
    const prefs = (hs && hs.profile_keys) || {};
    const rows = [];

    if (hs && hs.profile_source) {
      rows.push(row(t("agent-row-config-location") || "Config location", detailText(hs.profile_source, { mono: true })));
    }
    if (host.kind) { rows.push(row(t("host-kind") || "Host kind", detailText(host.kind, { mono: true }))); }
    if (host.config_format) { rows.push(row(t("host-config-format") || "Config format", detailText(host.config_format, { mono: true }))); }
    if (host.install_action_label) { rows.push(row(t("host-install-label") || "Install action", detailText(host.install_action_label))); }
    if (lastGen) {
      rows.push(row(t("host-last-generated") || "Last generated",
        detailText(lastGen.path, { mono: true }) + detailText(`${(lastGen.bytes / 1024).toFixed(1)} KB`, { muted: true })));
      if (lastGen.profile_uuid) { rows.push(row(t("host-profile-uuid") || "Profile UUID", detailText(lastGen.profile_uuid, { mono: true }))); }
      if (lastGen.payload_uuid) { rows.push(row(t("host-payload-uuid") || "Payload UUID", detailText(lastGen.payload_uuid, { mono: true }))); }
    }
    const prefsText = Object.keys(prefs).length === 0
      ? (t("host-prefs-empty") || "(none)")
      : Object.entries(prefs).map(([k, v]) => `${k} = ${v}`).join("\n");
    const prefsBlock = `<details class="sp-status__prefs"><summary>${escapeHtml(t("host-resolved-keys") || "Resolved profile keys")}</summary><pre class="sp-log">${escapeHtml(prefsText)}</pre></details>`;

    if (rows.length === 0) {
      return this._section("agent-section-config", "Technical detail", prefsBlock, true);
    }
    return this._section("agent-section-config", "Technical detail",
      `<table class="sp-status__board"><tbody>${rows.join("")}</tbody></table>${prefsBlock}`, true);
  }

  /** Sections are plain headings; the technical one is a collapsed disclosure. */
  _section(l10nId, fallback, body, collapsible = false) {
    const heading = t(l10nId) || fallback;
    if (collapsible) {
      return `<details class="sp-drawer__section sp-drawer__section--advanced">
        <summary class="sp-drawer__section-title">${escapeHtml(heading)}</summary>
        ${body}
      </details>`;
    }
    return `<section class="sp-drawer__section">
      <h3 class="sp-drawer__section-title">${escapeHtml(heading)}</h3>
      ${body}
    </section>`;
  }

  afterRender() {
    if (this._needsFocus && this.mode) {
      this._needsFocus = false;
      const close = this.querySelector(".sp-drawer__close");
      if (close) { close.focus(); }
    }
  }
}

reactive(SpAgentDrawer.prototype, ["mode", "hostId", "hosts", "snapshot", "gated", "busyId", "filterDraft", "confirmRemove"]);
customElements.define("sp-agent-drawer", SpAgentDrawer);
