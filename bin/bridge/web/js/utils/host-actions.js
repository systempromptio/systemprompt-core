// The host actions the UI actually offers, in the reader's vocabulary.
//
// "Generate" then "Install" is one intention split across two buttons: generate
// writes a profile to a temp path, install applies it. Nobody wants half of
// that. `repairHost` is the whole sequence — and it is also exactly what
// "Add this agent" means, because setting an agent up and repairing one are the
// same three calls.
//
// Extracted from sp-setup-agents.js so the wizard and the Agents tab cannot
// drift apart on error handling or on the probe that follows.

import { bridge } from "/assets/js/bridge.js";
import { pickCoworkSession } from "/assets/js/utils/cowork-session-picker.js";

export class HostActionError extends Error {
  /** @param {"generate"|"install"|"probe"} stage */
  constructor(stage, cause) {
    super(`${stage} failed: ${(cause && cause.message) || String(cause)}`);
    this.name = "HostActionError";
    this.stage = stage;
    this.cause = cause;
  }
}

/**
 * Generate this host's configuration profile, install it, then re-probe so the
 * UI reflects the result rather than waiting for the next tick.
 * @throws {HostActionError} carrying the stage that failed.
 */
const installWarnings = new Map();

export function hostInstallMessage(hostId, completedMessage) {
  return installWarnings.get(hostId) || completedMessage;
}

export async function repairHost(hostId) {
  let generated;
  try {
    generated = await bridge.hostProfileGenerate(hostId);
  } catch (e) {
    throw new HostActionError("generate", e);
  }
  const path = generated && (generated.path || generated.profile_path);
  if (!path) {
    throw new HostActionError("generate", new Error("generate did not return a path"));
  }
  try {
    const result = await bridge.hostProfileInstall(hostId, path);
    const profileWarnings = result && Array.isArray(result.warnings) ? result.warnings : [];
    installWarnings.set(hostId, profileWarnings.join(" "));

  } catch (e) {
    throw new HostActionError("install", e);
  }
  // A failed probe leaves a correctly installed profile in place; it only means
  // the display is stale, so it must not be reported as an install failure.
  try {
    await bridge.hostProbe(hostId);
  } catch (e) {
    console.warn("probe after repair", e);
  }
  return path;
}

export async function verifyHost(hostId) {
  return bridge.hostProbe(hostId);
}

export async function openHost(hostId) {
  return bridge.agentOpen(hostId);
}

export async function openHostConfig(hostId) {
  return bridge.agentOpenConfig(hostId);
}

export async function downloadHost(host) {
  const url = host && host.download_url;
  if (!url) { return; }
  return bridge.openExternalUrl(url);
}

// macOS gives no event when the user approves a profile in System Settings,
// so after opening the pane the host is re-probed until a profile is read back
// (or the user plainly walked away).
const APPROVAL_POLL_MS = 3000;
const APPROVAL_POLL_LIMIT = 60;

async function watchForApproval(hostId) {
  for (let i = 0; i < APPROVAL_POLL_LIMIT; i += 1) {
    await new Promise((resolve) => { setTimeout(resolve, APPROVAL_POLL_MS); });
    try {
      const reply = await bridge.hostProbe(hostId);
      const verdict = reply && reply.snapshot && reply.snapshot.verdict;
      if (verdict && verdict.is_set_up === true) { return; }
    } catch (e) {
      console.warn("probe while awaiting approval", e);
    }
  }
}

/** Settings → Personal subscriptions, where the user stores their own key. */
export function openPersonalKeys() {
  document.querySelector('.sp-rail-tab[data-tab="settings"]')?.click();
  requestAnimationFrame(() => {
    document.querySelector("sp-personal-keys")?.scrollIntoView({ block: "start", behavior: "smooth" });
  });
}

export async function openDeviceManagement(hostId) {
  await bridge.openDeviceManagement();
  watchForApproval(hostId);
}

/**
 * Run one of the actions the verdict recommends. Kept here so every surface
 * that renders a recommended button dispatches it identically.
 */
export async function runHostAction(kind, host) {
  const id = host && host.id;
  if (!id) { return; }
  switch (kind) {
    case "repair":
    case "update":
    case "update-admin":
    case "add":
    case "connect":
    case "offer-again": return repairHost(id);
    case "show-profile-file": return bridge.agentRevealProfile(id);
    case "open-device-management": return openDeviceManagement(id);
    case "restart":  return bridge.agentRestart(id);
    case "remove-leftover": return bridge.agentUninstall(id);
    case "verify":   return verifyHost(id);
    case "open":     return openHost(id);
    case "download": return downloadHost(host);
    case "enrol":    return bridge.agentEnrol(id);
    case "add-personal-key": return openPersonalKeys();
    case "pick-cowork-session": return pickCoworkSession();
    default:         return undefined;
  }
}
