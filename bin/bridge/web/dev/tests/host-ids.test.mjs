// The web tree spells host ids in one module, and every fixture the preview
// renders names only those ids. A host added to `HostKind` without a mirror
// entry would render without a logo or a label and be invisible to a reader of
// the page; a fixture naming an id the Rust side dropped would preview a host
// the shipped app can never show.
import test from "node:test";
import assert from "node:assert/strict";

import { HOST_ID, HOST_IDS } from "/assets/js/utils/host-ids.js";
import { fixture, fixtureNames } from "./fixtures.mjs";

const KINDS = new Set(["desktop-app", "cli-tool"]);

test("the mirror lists seven distinct kebab-case ids", () => {
  assert.equal(HOST_IDS.length, 7);
  assert.equal(new Set(HOST_IDS).size, 7);
  for (const id of HOST_IDS) {
    assert.match(id, /^[a-z]+(-[a-z]+)*$/, `${id} is not a kebab-case host id`);
  }
  assert.ok(Object.isFrozen(HOST_ID) && Object.isFrozen(HOST_IDS));
});

test("app and CLI packagings of a product are both listed", () => {
  assert.equal(HOST_ID.codex, "codex");
  assert.equal(HOST_ID.codexCli, "codex-cli");
  assert.equal(HOST_ID.opencode, "opencode");
  assert.equal(HOST_ID.opencodeCli, "opencode-cli");
});

test("every fixture host is a known id with a known packaging", () => {
  for (const name of fixtureNames()) {
    for (const host of fixture(name).host_apps || []) {
      assert.ok(HOST_IDS.includes(host.id), `${name}.json names unknown host '${host.id}'`);
      assert.ok(KINDS.has(host.kind), `${name}.json: ${host.id} has kind '${host.kind}'`);
    }
  }
});

test("the setup page's healthy state lists all seven hosts with their packaging", () => {
  const hosts = fixture("healthy").host_apps;
  const ids = hosts.map((h) => h.id).sort();
  assert.deepEqual(ids, [...HOST_IDS].sort());
  const byId = new Map(hosts.map((h) => [h.id, h]));
  assert.equal(byId.get(HOST_ID.codex).kind, "desktop-app");
  assert.equal(byId.get(HOST_ID.codexCli).kind, "cli-tool");
  assert.equal(byId.get(HOST_ID.opencode).kind, "desktop-app");
  assert.equal(byId.get(HOST_ID.opencodeCli).kind, "cli-tool");
  assert.equal(byId.get(HOST_ID.hermes).kind, "cli-tool");
  for (const host of hosts) {
    assert.equal(host.enabled, true, `${host.id} is listed, so it is enabled`);
  }
});
