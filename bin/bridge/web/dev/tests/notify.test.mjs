import test from "node:test";
import assert from "node:assert/strict";

import { isReadable, errorText } from "/assets/js/utils/notify.js";

test("a message the bridge wrote for the reader is the primary line", () => {
  assert.equal(isReadable({ code: "unauthorized", message: "Sign in again." }), true);
  assert.equal(isReadable("Enter a URL."), true);
});

test("an underlying error's text is kept for Details, not the primary line", () => {
  const err = { code: "internal", message: "io: permission denied" };
  assert.equal(isReadable(err), false);
  assert.equal(errorText(err), "io: permission denied");
  assert.equal(isReadable(new Error("boom")), false);
});
