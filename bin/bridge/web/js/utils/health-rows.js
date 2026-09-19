// The Setup health table's row model: one row per thing that can be wrong,
// worst first, each carrying the action that answers it when one exists.
//
// Pure so it runs under node:test. The rows used to be built inside the
// component, where nothing could assert that a host failure the sync could
// not write without UAC actually offered the UAC-backed repair.

import { t } from "/assets/js/i18n.js";

const RANK = { err: 0, warn: 1, unknown: 2, probing: 3, ok: 4, info: 5 };

// A validation line's row tone by its level, where the level says more than
// the tone does: an info line is a fact about this machine, not a check of
// unknown outcome. Every other level renders the tone Rust folded it to.
const TONE_BY_LEVEL = { info: "info" };

// Host warnings the host's own repair answers: `permission_rules` is raised
// when no managed Claude Code settings file exists, and installing the
// profile is what creates it.
const REPAIRABLE_WARNINGS = new Set(["permission_rules"]);

function repairAction(hostId, elevated) {
  return {
    kind: "repair",
    hostId,
    label: elevated
      ? (t("toast-action-repair-admin") || "Repair as administrator")
      : (t("agent-action-repair") || "Repair"),
  };
}

function firstLine(text) {
  const s = text == null ? "" : String(text);
  const nl = s.indexOf("\n");
  return nl === -1 ? s : s.slice(0, nl);
}

export function healthRows(snapshot) {
  const rows = [];
  const report = snapshot && snapshot.last_validation;
  for (const line of (report && report.lines) || []) {
    const tone = TONE_BY_LEVEL[line.level] || line.tone;
    rows.push({ tone, label: line.label, value: line.value, action: null });
  }
  for (const p of (snapshot && snapshot.provider_health) || []) {
    if (p.configured) { continue; }
    rows.push({
      tone: "warn",
      label: p.name,
      value: p.config_issue || (t("setup-health-provider-unconfigured") || "not configured"),
      action: null,
    });
  }
  for (const f of (snapshot && snapshot.startup_faults) || []) {
    rows.push({ tone: "err", label: `${t("setup-health-startup") || "startup"}: ${f.component}`, value: f.error, action: null });
  }
  if (snapshot && snapshot.credential_error) {
    rows.push({ tone: "err", label: t("setup-health-credential") || "credential", value: snapshot.credential_error, action: null });
  }
  const malformed = snapshot && snapshot.malformed_plugin_count;
  if (malformed) {
    rows.push({
      tone: "err",
      label: t("setup-health-malformed-plugins") || "malformed plugins",
      value: String(malformed),
      action: null,
    });
  }
  const sync = (snapshot && snapshot.last_sync_report) || {};
  for (const f of sync.host_failures || []) {
    rows.push({
      tone: "err",
      label: f.host_id,
      value: firstLine(f.error),
      action: repairAction(f.host_id, f.needs_elevation === true),
    });
  }
  for (const w of sync.host_warnings || []) {
    rows.push({
      tone: "warn",
      label: w.host_id,
      value: firstLine(w.message),
      action: REPAIRABLE_WARNINGS.has(w.kind) ? repairAction(w.host_id, false) : null,
    });
  }
  for (const d of sync.diagnostics || []) {
    rows.push({ tone: "warn", label: t("setup-health-diagnostic") || "gateway diagnostic", value: d, action: null });
  }
  rows.sort((a, b) => (RANK[a.tone] ?? 5) - (RANK[b.tone] ?? 5));
  return rows;
}

/** Whether the "Failures only" filter keeps a row. */
export function isFailure(row) {
  return row.tone === "err" || row.tone === "warn";
}
