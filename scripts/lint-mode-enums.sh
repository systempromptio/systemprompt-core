#!/usr/bin/env bash
# One enforcement vocabulary and one service-status vocabulary.
#
# `EnforcementMode {Enforce, Warn}` lives in
# crates/shared/manifest/src/services/enforcement.rs; any other `pub enum`
# whose unit-variant set is exactly {Enforce, Warn} is a copy and fails.
# `ServiceStatus {Starting, Running, Stopping, Stopped, Error}` lives in
# crates/shared/manifest/src/services/registry.rs; any other `pub enum` named
# `*Status` whose unit variants are a subset of that set is a copy and fails.
# Enums carrying payloads are observations, not vocabularies, and are skipped.
set -euo pipefail

cd "$(dirname "$0")/.."

python3 - <(git ls-files -co --exclude-standard 'crates/*/*/src/**/*.rs' 'crates/*/*/src/*.rs' \
    'bin/bridge/src/**/*.rs' 'bin/bridge/src/*.rs' \
    | grep -v '^crates/tests/' | sort -u) <<'PY'
import re
import sys

MODE_HOME = "crates/shared/manifest/src/services/enforcement.rs"
STATUS_HOME = "crates/shared/manifest/src/services/registry.rs"
SERVICE_SET = {"Starting", "Running", "Stopping", "Stopped", "Error"}


def enum_bodies(src):
    for m in re.finditer(r"pub enum (\w+)\s*\{", src):
        depth, i = 0, m.end() - 1
        while i < len(src):
            if src[i] == "{":
                depth += 1
            elif src[i] == "}":
                depth -= 1
                if depth == 0:
                    break
            i += 1
        yield m.group(1), src[m.end():i], m.start()


def unit_variants(body):
    body = re.sub(r"#\[[^\]]*\]", "", body)
    body = re.sub(r"//[^\n]*", "", body)
    variants, depth, item = set(), 0, ""
    for ch in body + ",":
        if ch in "{(":
            depth += 1
        elif ch in "})":
            depth -= 1
        if ch == "," and depth == 0:
            line = item.strip()
            item = ""
            if not line:
                continue
            ident = re.match(r"([A-Z]\w*)\s*(.*)$", line, re.S)
            if not ident:
                return None
            tail = ident.group(2).strip()
            if tail and not tail.startswith("="):
                return None
            variants.add(ident.group(1))
        else:
            item += ch
    return variants or None


failed = False
for path in (line.strip() for line in open(sys.argv[1], encoding="utf-8") if line.strip()):
    text = open(path, encoding="utf-8").read()
    if "pub enum" not in text:
        continue
    for name, body, start in enum_bodies(text):
        variants = unit_variants(body)
        if variants is None:
            continue
        lineno = text.count("\n", 0, start) + 1
        if variants == {"Enforce", "Warn"} and path != MODE_HOME:
            print(f"lint-mode-enums: {path}:{lineno} -- `{name}` duplicates EnforcementMode; use systemprompt_manifest::EnforcementMode", file=sys.stderr)
            failed = True
        if name.endswith("Status") and variants <= SERVICE_SET and path != STATUS_HOME:
            print(f"lint-mode-enums: {path}:{lineno} -- `{name}` duplicates ServiceStatus; use systemprompt_manifest::ServiceStatus", file=sys.stderr)
            failed = True

if failed:
    sys.exit(1)
print("lint-mode-enums: OK (one EnforcementMode, one ServiceStatus)")
PY
