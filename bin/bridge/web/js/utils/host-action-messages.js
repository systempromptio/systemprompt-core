// The toast each host action reports on success, shared by the Agents row,
// the drawer and the Agents banner so the three never word it differently.

import { t } from "/assets/js/i18n.js";

/**
 * The toast for an action that succeeded, or "" when the refreshed row says
 * enough on its own. Shared by the row and the drawer so they report alike.
 */
export function hostActionSuccessLine(kind, name, result) {
  const path = typeof result === "string" ? result : "";
  switch (kind) {
    case "repair":
    case "add":
    case "offer-again": return t("toast-agent-offered-again", { name })
      || `${name} profile offered again. Approve it within 8 minutes in Device Management.`;
    case "connect": return t("toast-agent-repaired", { name, path })
      || `${name} re-configured — wrote ${path}. Restart ${name} to pick it up.`;
    case "update":
    case "update-admin": return t("toast-agent-updated", { name, path })
      || `${name} updated — wrote ${path}. Quit and relaunch ${name} to load the new settings.`;
    case "enrol": return t("toast-agent-enrolled", { name })
      || `${name} now routes through the gateway. Restart ${name} to pick it up.`;
    case "verify": return t("toast-agent-verified", { name }) || `${name} re-checked.`;
    case "open-device-management": return t("toast-agent-approval-opened", { name })
      || `Approve the ${name} profile in Device Management. This list updates once you do.`;
    case "restart": return t("toast-agent-restarted", { name }) || `${name} is restarting.`;
    case "remove-leftover": return t("toast-agent-leftover-removed", { name })
      || `Removed the leftover ${name} settings.`;
    default: return "";
  }
}
