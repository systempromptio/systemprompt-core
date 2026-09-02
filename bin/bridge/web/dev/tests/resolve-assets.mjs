// Resolver for node:test runs of the GUI modules.
//
// The GUI is served at runtime with `/assets/js/...` as the module root, so
// every import in web/js is an absolute URL path. Node has no such root, so a
// bare `node --test` cannot load a single one of these files. This hook maps
// that prefix onto web/js/ and nothing else, which keeps the modules exactly as
// they ship — the point is to test the code that runs, not a copy adjusted to
// be testable.
//
// Register with: node --import ./web/dev/tests/resolve-assets.mjs --test ...
import { pathToFileURL } from "node:url";
import { dirname, resolve as resolvePath } from "node:path";
import { fileURLToPath } from "node:url";
import { register } from "node:module";

const HERE = dirname(fileURLToPath(import.meta.url));
export const WEB_ROOT = resolvePath(HERE, "../..");
const PREFIX = "/assets/js/";

export function resolve(specifier, context, nextResolve) {
  if (specifier.startsWith(PREFIX)) {
    const file = resolvePath(WEB_ROOT, "js", specifier.slice(PREFIX.length));
    return { url: pathToFileURL(file).href, shortCircuit: true };
  }
  return nextResolve(specifier, context);
}

register(import.meta.url, pathToFileURL(`${HERE}/`));
