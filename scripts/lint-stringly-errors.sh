#!/usr/bin/env bash
# Stringly-typed errors in production crates: an error turned into a `String`
# loses its source chain, its variant and its retry/status classification.
#
#   map-err-string  a `map_err(|e| …)` closure that stringifies `e`
#                   (`e.to_string()`, `format!("…{e}…")`, `format!("…", e)`),
#                   or `map_err(ToString::to_string)`.
#   result-string   `Result<_, String>` in a signature or binding.
#   string-variant  an error-enum variant whose only payload is a `String`
#                   (`Foo(String)`, `Foo { message: String }`) and which some
#                   production crate builds from an error's Display
#                   (`Enum::Foo(e.to_string())`, `Self::Foo(format!("{err}"))`).
#
# The typed form carries the cause: `Foo(#[from] Cause)` or
# `Foo { context: String, #[source] source: Cause }`. A string variant whose
# text is always authored by the caller is not reported.
#
# Usage:
#   lint-stringly-errors.sh            list every hit, exit 1 when any remain
#   lint-stringly-errors.sh --report   per-crate counts by category, exit 0
#   lint-stringly-errors.sh [--report] PATH...   report only hits under PATHs
#                                    (constructions are always read workspace-wide)
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
exec python3 - "$@" <<'PY'
import os
import re
import sys
from collections import defaultdict

args = sys.argv[1:]
report = False
if args and args[0] == "--report":
    report = True
    args = args[1:]
roots = [
    "crates/shared", "crates/infra", "crates/domain", "crates/app",
    "crates/entry", "systemprompt/src", "bin/bridge/src",
]
selected = [a.rstrip("/") for a in args] or roots


def is_selected(path):
    return any(path == s or path.startswith(s + "/") for s in selected)

MAP_ERR = re.compile(
    r"map_err\(\s*(?:"
    r"\|\s*(?P<var>\w+)\s*(?::[^|]*)?\|(?P<body>[^;]{0,400}?(?:"
    r"\b(?P=var)\.to_string\(\)"
    r"|format!\(\"(?:[^\"\\]|\\.)*\{(?P=var)[:}]"
    r"|format!\(\"(?:[^\"\\]|\\.)*\"(?:\s*,\s*(?:[^,;()]|\([^()]*\))+)*?\s*,\s*&?(?P=var)\s*\)"
    r"))"
    r"|(?:std::string::)?ToString::to_string\s*\)"
    r")",
    re.S,
)
RESULT_STRING = re.compile(
    r"\bResult<(?:[^<>;]|<(?:[^<>;]|<[^<>;]*>)*>)*,\s*(?:std::string::)?String\s*>"
)
STRING_VARIANT = re.compile(
    r"^\s*(?:#\[error\([^)]*\)\]\s*)?(?P<name>[A-Z]\w*)\s*"
    r"(?:\(\s*(?:::)?(?:std::string::)?String\s*\)"
    r"|\{\s*\w+\s*:\s*(?:::)?(?:std::string::)?String\s*,?\s*\})\s*,?\s*$"
)
ERR_IDENT = r"(?:e|err|error|source|cause|inner|\w+_err|\w+_error)"
BUILT = re.compile(
    r"\b(?P<enum>[A-Z]\w*|Self)::(?P<name>[A-Z]\w*)\s*\(\s*(?:"
    + ERR_IDENT
    + r"\.to_string\(\)|format!\(\"(?:[^\"\\]|\\.)*\{"
    + ERR_IDENT
    + r"[:}]|format!\(\"(?:[^\"\\]|\\.)*\"[^;]*?,\s*&?"
    + ERR_IDENT
    + r"\s*\))"
)
ENUM_DECL = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?enum\s+(?P<name>[A-Z]\w*)")
IMPL_DECL = re.compile(
    r"^\s*impl(?:<[^{]*?>)?\s+(?:[\w:]+(?:<[^{]*?>)?\s+for\s+)?(?:[\w]+::)*(?P<name>[A-Z]\w*)"
)
ERROR_DERIVE = re.compile(r"thiserror::Error|derive\([^)]*\bError\b|domain_error!")


def crate_of(path):
    parts = path.split("/")
    if parts[0] == "crates":
        return "/".join(parts[:3])
    if parts[0] == "bin":
        return "/".join(parts[:2])
    return parts[0]


def production_files():
    for base in roots:
        if not os.path.exists(base):
            continue
        for dirpath, dirnames, filenames in os.walk(base):
            dirnames[:] = [d for d in dirnames if d not in ("tests", "target")]
            for name in filenames:
                if name.endswith(".rs") and name != "build.rs":
                    yield os.path.join(dirpath, name)


hits = []
built_from_display = set()
variant_lines = []

for path in sorted(production_files()):
    with open(path, encoding="utf-8") as handle:
        text = handle.read()
    for match in MAP_ERR.finditer(text):
        line = text.count("\n", 0, match.start()) + 1
        hits.append(("map-err-string", path, line))
    lines = text.splitlines()
    for number, line in enumerate(lines, 1):
        if RESULT_STRING.search(line):
            hits.append(("result-string", path, number))
    current_impl = None
    offsets = [0]
    for line in lines:
        offsets.append(offsets[-1] + len(line) + 1)
    impl_at = []
    for number, line in enumerate(lines):
        found = IMPL_DECL.match(line)
        if found:
            current_impl = found.group("name")
        impl_at.append(current_impl)
    for built in BUILT.finditer(text):
        enum = built.group("enum")
        if enum == "Self":
            row = text.count("\n", 0, built.start())
            enum = impl_at[row] if row < len(impl_at) else None
        if enum:
            built_from_display.add((enum, built.group("name")))
    if ERROR_DERIVE.search(text):
        current_enum = None
        for number, line in enumerate(lines, 1):
            declared = ENUM_DECL.match(line)
            if declared:
                current_enum = declared.group("name")
                continue
            found = STRING_VARIANT.match(line)
            if found and current_enum:
                variant_lines.append((current_enum, found.group("name"), path, number))

for enum, name, path, number in variant_lines:
    if (enum, name) in built_from_display:
        hits.append(("string-variant", path, number))

hits = [h for h in hits if is_selected(h[1])]

categories = ("map-err-string", "result-string", "string-variant")

if report:
    if not hits:
        print("lint-stringly-errors: no stringly errors")
        sys.exit(0)
    counts = defaultdict(lambda: defaultdict(int))
    for category, path, _ in hits:
        counts[crate_of(path)][category] += 1
    rows = sorted(counts.items(), key=lambda kv: -sum(kv[1].values()))
    print(f"{'crate':<34} {'map-err-string':>15} {'result-string':>14} {'string-variant':>15} {'total':>7}")
    totals = defaultdict(int)
    for crate, by_category in rows:
        values = [by_category[c] for c in categories]
        for c, v in zip(categories, values):
            totals[c] += v
        print(f"{crate:<34} {values[0]:>15} {values[1]:>14} {values[2]:>15} {sum(values):>7}")
    values = [totals[c] for c in categories]
    print(f"{'TOTAL':<34} {values[0]:>15} {values[1]:>14} {values[2]:>15} {sum(values):>7}")
    sys.exit(0)

if hits:
    print("lint-stringly-errors: stringly-typed errors in production crates.")
    print("Keep the cause in a #[from]/#[source] variant instead of e.to_string():")
    for category, path, line in sorted(hits, key=lambda h: (h[1], h[2])):
        print(f"{category}\t{path}:{line}")
    sys.exit(1)
PY
