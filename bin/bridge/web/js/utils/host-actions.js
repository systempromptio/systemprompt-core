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
    await bridge.hostProfileInstall(hostId, path);
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

/**
 * Run one of the actions the verdict recommends. Kept here so every surface
 * that renders a recommended button dispatches it identically.
 */
export async function runHostAction(kind, host) {
  const id = host && host.id;
  if (!id) { return; }
  switch (kind) {
    case "repair":
    case "add":      return repairHost(id);
    case "verify":   return verifyHost(id);
    case "open":     return openHost(id);
    case "download": return downloadHost(host);
    default:         return undefined;
  }
}
