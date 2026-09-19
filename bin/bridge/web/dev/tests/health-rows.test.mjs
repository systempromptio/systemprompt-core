// The Setup health table's row model.
//
// This file exists because a sync that could not write the machine-wide
// Claude Desktop policy without UAC listed the host as "down" and offered
// nothing to press: the repair lived only on an error toast whose button a
// de-duplicated second toast had just wiped. The rows now carry the action.
import test from "node:test";
import assert from "node:assert/strict";

import { healthRows, isFailure } from "/assets/js/utils/health-rows.js";
import { fixture } from "./fixtures.mjs";

function withSync(sync) {
  return { ...fixture("healthy"), last_sync_report: { host_failures: [], host_warnings: [], diagnostics: [], malformed: [], ...sync } };
}

test("a host failure that needs elevation offers the administrator repair", () => {
  const rows = healthRows(withSync({
    host_failures: [{
      host_id: "claude-desktop",
      emitter: "policy",
      error: "the Claude Desktop machine policy needs administrator approval: HKLM\\SOFTWARE\\Policies\\Claude holds different values for managedMcpServers\ncaused by: access denied",
      needs_elevation: true,
    }],
  }));
  const row = rows.find((r) => r.label === "claude-desktop");
  assert.ok(row, "the failing host is a row");
  assert.equal(row.tone, "err");
  assert.equal(rows.indexOf(row), 0, "failures sort first");
  assert.ok(row.action, "a host failure carries its repair");
  assert.equal(row.action.kind, "repair");
  assert.equal(row.action.hostId, "claude-desktop");
  assert.match(row.action.label, /administrator/i);
  assert.ok(!row.value.includes("\n"), "only the first line is shown");
});

test("a host failure without elevation offers a plain repair", () => {
  const rows = healthRows(withSync({
    host_failures: [{ host_id: "opencode", emitter: "config", error: "write failed", needs_elevation: false }],
  }));
  const row = rows.find((r) => r.label === "opencode");
  assert.ok(row.action);
  assert.doesNotMatch(row.action.label, /administrator/i);
});

test("the missing-settings-file warning is repairable; an evidence warning is not", () => {
  const rows = healthRows(withSync({
    host_warnings: [
      { kind: "permission_rules", host_id: "claude-code", message: "10 tool permission rules not applied: no Claude Code settings file is managed by this bridge" },
      { kind: "evidence_unacknowledged", host_id: "claude-code", message: "Installation evidence unacknowledged: feedback request rejected with status 409" },
    ],
  }));
  const [rules, evidence] = rows.filter((r) => r.label === "claude-code");
  assert.equal(rules.tone, "warn");
  assert.equal(rules.action && rules.action.hostId, "claude-code");
  assert.equal(evidence.action, null, "nothing on this machine answers a server-side conflict");
});

test("an info validation line is a fact, not an unknown check, and sorts last", () => {
  const snap = withSync({});
  snap.last_validation = {
    lines: [
      { level: "info", tone: "unknown", label: "binary", value: "astound-bridge v0.56.0 (windows-x86_64)" },
      { level: "ok", tone: "ok", label: "org-plugins path", value: "C:\\ProgramData" },
    ],
    any_failed: false,
  };
  const rows = healthRows(snap);
  const binary = rows.find((r) => r.label === "binary");
  assert.equal(binary.tone, "info");
  assert.equal(rows.at(-1), binary, "info lines sort after every check");
  assert.equal(isFailure(binary), false);
  assert.equal(binary.action, null);
});
