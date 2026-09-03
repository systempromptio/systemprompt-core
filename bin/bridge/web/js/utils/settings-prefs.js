import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";

/**
 * Inline validation for the gateway field. The wizard's own validation lived
 * behind a flow with no exit, so a user checking their URL could not reach it.
 */
export function gatewayUrlError(value) {
  if (!value) { return t("settings-gateway-empty") || "Enter a gateway URL."; }
  let parsed;
  try { parsed = new URL(value); } catch (_) {
    return t("settings-gateway-invalid") || "That is not a valid URL.";
  }
  if (parsed.protocol !== "https:" && parsed.hostname !== "localhost" && parsed.hostname !== "127.0.0.1") {
    return t("settings-gateway-https") || "The gateway must be https, except on localhost.";
  }
  return "";
}

export function loadPrefs(component) {
  bridge.settingsGet()
    .then((p) => { component.prefs = p; })
    .catch((e) => console.warn("settings read failed", e));
}
