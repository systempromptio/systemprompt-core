import { isInstalled } from "/assets/js/utils/verdict.js";

function isConfigured(snap) {
  return !!snap.signed_in;
}

/**
 * The wizard's step model from one snapshot, as a pure function of the
 * snapshot and the two latches the component carries between snapshots.
 * `setupMode` is `null` when this snapshot must not change the overlay.
 */
export function stepsFromSnapshot(snap, { leftSetup, finished }) {
  const configured = isConfigured(snap);
  const hosts = snap.host_apps || [];
  // Install state for a host is only KNOWN once its probe has completed, at
  // which point `snapshot` is populated. Until every host has a snapshot the
  // result is "unknown" — we must not show onboarding then, or it flashes
  // before detection resolves (the bug where it appeared with agents already
  // installed). Once settled, show the agents step only when none are
  // installed; installing one (anyInstalled) drops straight into the app.
  const settled = hosts.length > 0 && hosts.every((h) => h.snapshot);
  const anyInstalled = hosts.some(isInstalled);
  const model = {
    settled,
    anyInstalled,
    step: configured ? "agents" : "connect",
    firstRunActive: !!(snap.first_run && snap.first_run.active),
    leftSetup,
    setupMode: null,
  };
  // First-use provisioning pins the wizard open. Checked before the
  // settled/latched guards below: a run is exactly the window in which host
  // snapshots are still arriving, so those guards would return early and let
  // the app show over a half-installed machine.
  if (model.firstRunActive) { return { ...model, leftSetup: false, setupMode: true }; }
  return { ...model, ...overlayDecision(snap, { configured, settled, anyInstalled, leftSetup, finished }) };
}

function overlayDecision(snap, { configured, settled, anyInstalled, leftSetup, finished }) {
  // Signing out is the one thing that legitimately sends us back to the
  // splash. Clear the latch so it can.
  const signedIn = !!(snap.verified_identity && snap.verified_identity.user_id);
  const latch = signedIn ? leftSetup : false;
  // Everything below decides whether to show a full-screen overlay, so it must
  // only ever run on a settled snapshot. `configured` and `anyInstalled` each
  // start out false and flip true as the gateway probe and then the host
  // probes land — evaluating on those partial snapshots is what made the
  // window flick splash → app → splash → app during startup.
  const gatewayProbing = !snap.gateway_status || !snap.gateway_status.settled;
  if (gatewayProbing || !settled) { return { leftSetup: latch, setupMode: null }; }
  // One-way latch: once the app proper has been shown, a later probe result
  // must not yank the user back into onboarding mid-session.
  if (latch) { return { leftSetup: true, setupMode: null }; }
  const needAgents = !anyInstalled && !finished;
  const inSetup = !configured || needAgents;
  return { leftSetup: !inSetup, setupMode: inSetup };
}
