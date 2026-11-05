import test from "node:test";
import assert from "node:assert/strict";

import { gatewayOutageLine } from "/assets/js/utils/gateway.js";
import { fixture } from "./fixtures.mjs";

test("the outage fixture carries the line and a healthy snapshot does not", () => {
  assert.notEqual(gatewayOutageLine(fixture("gateway-outage")), "");
  assert.equal(gatewayOutageLine(fixture("healthy")), "");
});
