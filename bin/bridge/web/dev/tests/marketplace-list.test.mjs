import test from "node:test";
import assert from "node:assert/strict";
import { placeholderMarkup } from "/assets/js/components/marketplace-list-placeholder.js";

// Rendering these components only needs the element's event interface. The
// browser smoke check exercises connection, reconciliation and child updates.
globalThis.HTMLElement = class extends EventTarget {
  querySelectorAll() { return []; }
};
globalThis.window = {};
globalThis.customElements = { define() {} };
const { SpMarketplaceList } = await import("/assets/js/components/sp-marketplace-list.js");

test("signed-out and disconnected states are explicit rather than loading skeletons", () => {
  for (const [reason, text] of [
    ["signed-out", "Sign in"],
    ["verifying", "Checking your connection"],
    ["gateway-unreachable", "Waiting for the gateway"],
  ]) {
    const html = placeholderMarkup({ state: "idle", reason });
    assert.ok(html.includes(text));
    assert.ok(!html.includes("skeleton"));
  }
  assert.match(placeholderMarkup({ state: "loading" }), /skeleton/);
});

test("refresh errors display a retry action alongside existing skills", () => {
  const list = new SpMarketplaceList();
  list.state = "ok";
  list.items = [{ id: "retained-skill", name: "Retained skill" }];
  list.error = "A <read> failed";
  const html = list.render();
  assert.match(html, /Showing the previous list/);
  assert.match(html, /data-action="retry"/);
  assert.match(html, /A &lt;read&gt; failed/);
  assert.match(html, /Retained skill/);
  assert.ok(!html.includes("skeleton"));
});

test("an empty cached listing still exposes refresh failures and Retry", () => {
  const list = new SpMarketplaceList();
  list.state = "ok";
  list.kind = "skills";
  list.error = "Read failed";
  const html = list.render();
  assert.match(html, /No skills yet/);
  assert.match(html, /data-action="retry"/);
});
