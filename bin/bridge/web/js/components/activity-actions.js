import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { runAction } from "/assets/js/utils/action.js";

function copyActivityLog(component) {
  const rows = component._virtual ? component._virtual.entries() : component._entries;
  return navigator.clipboard.writeText(rows.map((r) => r.text).join("\n"));
}

export function registerActivityToolActions(component) {
  component.registerAction("open-log-folder", (trigger) => runAction(trigger, {
    run: () => bridge.openLogFolder(),
    context: t("activity-open-log-folder") || "Open log folder",
  }));
  component.registerAction("export-bundle", (trigger) => runAction(trigger, {
    run: () => bridge.diagnosticsExportBundle(),
    success: (v) => (v && v.path)
      ? (t("activity-bundle-written", { path: v.path }) || `Diagnostic bundle written to ${v.path}`)
      : (t("activity-bundle-done") || "Diagnostic bundle written."),
    context: t("activity-export-bundle") || "Export diagnostic bundle",
  }));
  component.registerAction("copy", (trigger) => runAction(trigger, {
    run: () => copyActivityLog(component),
    success: t("activity-copied") || "Copied",
    context: t("activity-copy") || "Copy",
  }));
}
