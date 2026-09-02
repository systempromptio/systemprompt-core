// The onboarding wizard's step model.
//
// This file exists because of a bug that shipped in v0.43.0: the settle gate
// read `h.snapshot`, a field the wire stopped carrying when the host payload
// was reduced to `health`. The expression was not merely wrong, it was constant
// `false` — so the 12s settle timer always fired and every sign-in ended on
// "Still checking this computer. Some agents have not reported yet." with a
// "Continue anyway" button. No test could have failed, because the whole GUI
// layer had none.
import test from "node:test";
import assert from "node:assert/strict";

import { stepsFromSnapshot } from "/assets/js/utils/setup-steps.js";
import { fixture, fixtureNames } from "./fixtures.mjs";

const LATCHES = { leftSetup: false, finished: false };

test("a healthy snapshot settles", () => {
  const model = stepsFromSnapshot(fixture("healthy"), LATCHES);
  assert.equal(
    model.settled,
    true,
    "every host in healthy.json has reported, so the wizard must settle — " +
      "an unsettleable gate is what produced the 'Continue anyway' screen",
  );
});

test("settling does not depend on a field the wire never sends", () => {
  // The guard for the actual defect, stated as the property rather than the
  // instance: reading any absent field yields undefined, so a gate built on one
  // can never pass no matter what the host reports.
  const snap = fixture("healthy");
  for (const host of snap.host_apps) {
    assert.ok(
      !("snapshot" in host),
      `host '${host.id}' carries a 'snapshot' key; the settle gate must not ` +
        "be rebuilt on it — the wire dropped it in favour of 'health'",
    );
  }
});

test("a host that has not reported keeps the wizard unsettled", () => {
  const snap = fixture("healthy");
  const target = snap.host_apps.find((h) => h.health);
  assert.ok(target, "healthy.json must contain a host with health to blank");
  target.health = null;
  assert.equal(
    stepsFromSnapshot(snap, LATCHES).settled,
    false,
    "the gate must still detect a genuinely unreported host",
  );
});

test("every fixture produces a coherent step model", () => {
  for (const name of fixtureNames()) {
    const model = stepsFromSnapshot(fixture(name), LATCHES);
    assert.equal(typeof model.settled, "boolean", `${name}: settled`);
    assert.ok(
      ["connect", "agents"].includes(model.step),
      `${name}: unexpected step '${model.step}'`,
    );
  }
});
