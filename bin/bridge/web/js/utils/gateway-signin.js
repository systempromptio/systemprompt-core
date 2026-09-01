import { bridge } from "/assets/js/bridge.js";
import { notifyErr } from "/assets/js/utils/notify.js";
import { t } from "/assets/js/i18n.js";

const PENDING_TIMEOUT_MS = 15000;

function fail(component, message) {
  component.error = message;
  component.invalidate();
  return null;
}

export function validGatewayUrl(component) {
  const gw = (component.gateway || "").trim();
  if (!gw) { return fail(component, t("setup-gateway-required-url") || "Enter the gateway URL."); }
  if (!/^https?:\/\//i.test(gw)) {
    return fail(component, t("setup-gateway-scheme") || "Gateway URL must start with http:// or https://");
  }
  // A local gateway is plain HTTP; an https:// typo here is saved verbatim
  // and then opened in the browser, which fails with a TLS protocol error.
  if (/^https:\/\/(localhost|127\.0\.0\.1|\[::1\])(:|\/|$)/i.test(gw)) {
    return fail(component, t("setup-gateway-loopback-https")
      || 'A gateway on this machine is served over http://, not https:// — drop the "s".');
  }
  return gw;
}

export async function persistGatewayUrl(component) {
  const url = (component.gateway || "").trim();
  if (!url || url === component._lastSavedGateway) { return; }
  if (!/^https?:\/\//i.test(url)) {
    fail(component, t("setup-gateway-scheme") || "Gateway URL must start with http:// or https://");
    return;
  }
  component._lastSavedGateway = url;
  try {
    await bridge.gatewaySet(url);
  } catch (e) {
    component._lastSavedGateway = "";
    const context = t("setup-gateway-save-failed") || "Could not save the gateway URL";
    fail(component, `${context}: ${(e && e.message) || e}`);
    notifyErr(e, context);
  }
}

export async function signInToGateway(component) {
  const gw = validGatewayUrl(component);
  if (!gw) { return; }
  // Read the checkbox live so the choice is honored even if no change event fired.
  const keepEl = component.querySelector("#setup-keep");
  component.keepSignedIn = keepEl ? keepEl.checked : component.keepSignedIn;
  component._lastSavedGateway = gw;
  component.signingIn = true; component.error = ""; component.invalidate();
  try {
    await bridge.signIn(gw, component.keepSignedIn);
  } catch (err) {
    component.error = `Sign-in failed: ${(err && err.message) || err}`;
  } finally {
    component.signingIn = false; component.invalidate();
  }
}

export async function cancelGatewaySignIn(component) {
  try {
    await bridge.cancel("login");
  } catch (err) {
    console.warn("cancel sign-in", err);
  }
  component.signingIn = false; component.error = ""; component.invalidate();
}

export function clearGatewayPendingTimer(component) {
  if (component._pendingTimer) { clearTimeout(component._pendingTimer); component._pendingTimer = null; }
}

export function resolveGatewayPending(component) {
  component.pending = false; component._pendingSince = 0; clearGatewayPendingTimer(component);
}

function armPendingTimer(component) {
  clearGatewayPendingTimer(component);
  component._pendingTimer = setTimeout(() => {
    if (!component.pending) { return; }
    resolveGatewayPending(component);
    if (!component.error) { component.error = "Connection attempt timed out."; }
    component.invalidate();
  }, PENDING_TIMEOUT_MS);
}

export async function connectGatewayWithPat(component) {
  const gw = validGatewayUrl(component);
  if (!gw) { return; }
  component._lastSavedGateway = gw;
  component.pending = true; component._pendingSince = Date.now(); component.error = ""; component.invalidate();
  armPendingTimer(component);
  try {
    if (component.patSaved) { await bridge.gatewayProbe(); return; }
    const token = (component.pat || "").trim();
    if (!token) {
      resolveGatewayPending(component);
      fail(component, "Paste your personal access token.");
      return;
    }
    await bridge.login(token, gw);
  } catch (err) {
    component.error = `${component.patSaved ? "Probe" : "Login"} failed: ${(err && err.message) || err}`;
    resolveGatewayPending(component); component.invalidate();
  }
}
