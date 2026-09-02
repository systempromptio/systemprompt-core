// Load the same dev fixtures the browser preview serves, so a test and
// `just bridge-preview` are looking at the same bytes. `wire_hosts.rs`
// (every_fixture_host_entry_carries_exactly_the_wire_key_set) holds those bytes
// to the Rust wire shape, which is what makes them usable as a contract here.
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { WEB_ROOT } from "./resolve-assets.mjs";

const DIR = join(WEB_ROOT, "dev/fixtures");

export function fixture(name) {
  return JSON.parse(readFileSync(join(DIR, `${name}.json`), "utf8"));
}

export function fixtureNames() {
  return readdirSync(DIR)
    .filter((f) => f.endsWith(".json"))
    .map((f) => f.slice(0, -5))
    .sort();
}
