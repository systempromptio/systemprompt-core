import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { repairHost, runHostAction, openHostConfig } from "/assets/js/utils/host-actions.js";
import { notifyOk, notifyErr, notifyAction } from "/assets/js/utils/notify.js";
import { AGENT_WIRE_SURFACES, agentHostName } from "/assets/js/components/agent-drawer-parts.js";

// Repair and Add both rewrite a config file the user cannot see, and both need
// the agent restarted afterwards. Saying which file was written is the whole
// difference between "something happened" and a report.
function successLine(kind, host, result) {
  const name = agentHostName(host);
  switch (kind) {
    case "repair":
    case "add":    return t("toast-agent-repaired", { name, path: result || "" })
      || `${name} re-configured — wrote ${result || ""}. Restart ${name} to pick it up.`;
    case "verify": return t("toast-agent-verified", { name }) || `${name} re-checked.`;
    default:       return "";
  }
}

export async function runAgentDrawerAct(drawer, trigger) {
  const host = drawer._hostById(trigger.dataset.hostId) || drawer._host();
  const kind = trigger.dataset.kind;
  if (!host || !kind || drawer.busyId) { return; }
  drawer.busyId = host.id;
  try {
    const result = await runHostAction(kind, host);
    const line = successLine(kind, host, result);
    if (line) { notifyOk(line); }
  } catch (e) {
    notifyErr(e, t(`agent-action-${kind}`) || kind);
  } finally {
    drawer.busyId = null;
  }
}

export async function runAgentDrawerOpenConfig(drawer) {
  const host = drawer._host();
  if (!host) { return; }
  try { await openHostConfig(host.id); }
  catch (e) { notifyErr(e, t("agent-action-open-config") || "Show config file"); }
}

export async function runAgentDrawerAddHost(drawer, trigger) {
  const id = trigger.dataset.hostId;
  if (!id || drawer.busyId) { return; }
  drawer.busyId = id;
  const name = agentHostName(drawer._hostById(id));
  try {
    const path = await repairHost(id);
    notifyOk(t("toast-agent-added", { name, path })
      || `${name} added — wrote ${path}. Restart ${name} to pick it up.`);
  } catch (e) {
    notifyErr(e, t("agent-action-add") || "Add");
  } finally {
    drawer.busyId = null;
  }
}

export function captureAgentDrawerFilter(drawer) {
  const allEl = drawer.querySelector("[data-model-all]");
  const host = drawer._host();
  const saved = (host && Array.isArray(host.model_protocols)) ? host.model_protocols : [];
  // Surfaces with no checkbox are not the user's to change here, so they
  // survive the save rather than being deleted by omission.
  const unshown = saved.filter((tag) => !AGENT_WIRE_SURFACES.includes(tag));
  drawer.filterDraft = {
    all: allEl ? allEl.checked : false,
    protocols: Array.from(drawer.querySelectorAll("[data-proto]"))
      .filter((el) => el.checked)
      .map((el) => el.dataset.proto)
      .concat(unshown),
  };
}

async function writeModelFilter(drawer, trigger, protocols, okLine, errLabel) {
  trigger.disabled = true;
  try {
    await bridge.hostModelFilterSet(drawer._host().id, protocols);
    drawer.filterDraft = null;
    notifyOk(okLine);
  } catch (e) {
    notifyErr(e, errLabel);
  } finally {
    if (trigger.isConnected) { trigger.disabled = false; }
  }
}

export async function runAgentDrawerSaveFilter(drawer, trigger) {
  const draft = drawer.filterDraft;
  if (!drawer._host() || !draft) { return; }
  await writeModelFilter(drawer, trigger, draft.all ? [] : draft.protocols,
    t("toast-model-filter-saved") || "Model filter saved to your systemprompt account.",
    t("host-model-filter-save") || "Save filter");
}

export async function runAgentDrawerResetFilter(drawer, trigger) {
  if (!drawer._host()) { return; }
  await writeModelFilter(drawer, trigger, null,
    t("toast-model-filter-reset") || "Model filter reset to this agent's default.",
    t("host-model-filter-reset") || "Reset to default");
}

function removalOutcome(drawer, name, result) {
  // The reply distinguishes a removal from an instruction, because on macOS
  // the profile is held by the OS and only the user can withdraw it.
  if (result && result.removed) {
    notifyOk(t("toast-agent-removed", { name }) || `${name} removed. Restart it to drop the old settings.`);
    drawer.close();
    return;
  }
  notifyAction(result && result.instruction
    ? t("toast-agent-remove-manual", { name, instruction: result.instruction }) || `${name}: ${result.instruction}`
    : t("toast-agent-remove-nothing", { name }) || `${name} had nothing left to remove.`);
}

export async function runAgentDrawerConfirmRemove(drawer, trigger) {
  const host = drawer._host();
  if (!host || drawer.busyId) { return; }
  drawer.busyId = host.id;
  try {
    const result = await bridge.agentUninstall(host.id);
    drawer.confirmRemove = false;
    removalOutcome(drawer, agentHostName(host), result);
  } catch (e) {
    notifyErr(e, t("agent-action-remove") || "Remove agent");
  } finally {
    drawer.busyId = null;
    if (trigger.isConnected) { trigger.disabled = false; }
  }
}
