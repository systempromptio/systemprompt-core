#!/usr/bin/env python3
"""Changelog fragments: validate them, or assemble them into a release block.

One change is one file, `changelog.d/<id>.toml` (format: changelog.d/README.md).
A fragment holds at most one `[root]` entry, written to the root CHANGELOG.md
under its area label, and any number of `[[crate]]` entries, each written as a
plain bullet to that crate's own CHANGELOG.md.

  changelog-assemble.py --check
      validate every fragment: known crate, known section, area on the root
      entry, non-empty text, no `[Unreleased]`, unique id
  changelog-assemble.py --version X.Y.Z --date YYYY-MM-DD [--dry-run]
      write the `## [X.Y.Z] - date` block into the root CHANGELOG.md and into
      every named crate's CHANGELOG.md, then delete the consumed fragments;
      `--dry-run` prints the blocks and changes nothing
  changelog-assemble.py --named-crates
      validate, then print the package name of every crate a fragment names

Output is byte-stable: within a changelog, entries are ordered by section
(Breaking, Added, Changed, Fixed, Removed), then by fragment id, then by their
order inside the fragment. Python 3 standard library only.
"""
import argparse
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FRAGMENT_DIR = "changelog.d"
SECTIONS = ("Breaking", "Added", "Changed", "Fixed", "Removed")
ROOT_TARGET = "."
ID_RE = re.compile(r"^[a-z0-9][a-z0-9-]*$")
VERSION_RE = re.compile(r"^\d+\.\d+\.\d+$")
DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
KEY_RE = re.compile(r"^([A-Za-z_][A-Za-z0-9_-]*)\s*=\s*(.*)$")
ESCAPES = {'"': '"', "\\": "\\", "n": "\n", "t": "\t"}
ROOT_KEYS = {"section", "area", "text"}
CRATE_KEYS = {"name", "section", "text"}


class FragmentError(Exception):
    pass


def parse_string(raw, where):
    if not raw.startswith('"'):
        raise FragmentError(f"{where}: value must be a double-quoted string")
    out, i = [], 1
    while i < len(raw):
        char = raw[i]
        if char == "\\":
            if i + 1 >= len(raw):
                raise FragmentError(f"{where}: dangling escape")
            nxt = raw[i + 1]
            if nxt == "u" and re.match(r"[0-9A-Fa-f]{4}$", raw[i + 2:i + 6]):
                out.append(chr(int(raw[i + 2:i + 6], 16)))
                i += 6
                continue
            if nxt not in ESCAPES:
                raise FragmentError(f"{where}: unsupported escape \\{nxt}")
            out.append(ESCAPES[nxt])
            i += 2
            continue
        if char == '"':
            rest = raw[i + 1:].strip()
            if rest and not rest.startswith("#"):
                raise FragmentError(f"{where}: trailing text after the string")
            return "".join(out)
        out.append(char)
        i += 1
    raise FragmentError(f"{where}: unterminated string")


def parse_fragment(text, name):
    """The TOML subset fragments use: `[root]`, `[[crate]]`, `key = "string"`."""
    root, crates, table = None, [], None
    for number, raw in enumerate(text.splitlines(), 1):
        line = raw.strip()
        where = f"{name}:{number}"
        if not line or line.startswith("#"):
            continue
        if line == "[root]":
            if root is not None:
                raise FragmentError(f"{where}: a fragment holds at most one [root] entry")
            root = table = {}
            continue
        if line == "[[crate]]":
            table = {}
            crates.append(table)
            continue
        if line.startswith("["):
            raise FragmentError(f"{where}: unknown table {line}")
        match = KEY_RE.match(line)
        if not match:
            raise FragmentError(f"{where}: expected `key = \"value\"`")
        if table is None:
            raise FragmentError(f"{where}: key outside a [root] or [[crate]] table")
        key, value = match.group(1), parse_string(match.group(2).strip(), where)
        allowed = ROOT_KEYS if table is root else CRATE_KEYS
        if key not in allowed:
            raise FragmentError(f"{where}: unknown key {key!r} (allowed: {', '.join(sorted(allowed))})")
        if key in table:
            raise FragmentError(f"{where}: duplicate key {key!r}")
        table[key] = value
    return root, crates


def published_crates(root):
    """Package name -> directory of every published crate outside the test workspace."""
    listed = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "*Cargo.toml"],
        cwd=root, check=True, capture_output=True, text=True,
    ).stdout.split()
    crates = {}
    for manifest in sorted(listed):
        if manifest.startswith("crates/tests/") or not (root / manifest).is_file():
            continue
        text = (root / manifest).read_text()
        package = re.search(r"^\[package\]\s*$(.*?)(?=^\[|\Z)", text, re.M | re.S)
        if not package:
            continue
        body = package.group(1)
        if re.search(r"^publish\s*=\s*false", body, re.M):
            continue
        name = re.search(r'^name\s*=\s*"([^"]+)"', body, re.M)
        if name:
            crates[name.group(1)] = str(Path(manifest).parent)
    return crates


def load_fragments(root, crates):
    """Every fragment as (id, entries) sorted by id; raises with every problem found."""
    directory = root / FRAGMENT_DIR
    problems, fragments = [], []
    paths = sorted(directory.glob("*.toml")) if directory.is_dir() else []
    for path in paths:
        fid = path.stem
        if not ID_RE.match(fid):
            problems.append(f"{path.name}: id must be lowercase letters, digits and dashes")
            continue
        try:
            root_entry, crate_entries = parse_fragment(path.read_text(), path.name)
        except FragmentError as error:
            problems.append(str(error))
            continue
        entries = []
        if root_entry is not None:
            entries.append((ROOT_TARGET, root_entry))
        entries += [(entry.get("name", ""), entry) for entry in crate_entries]
        if not entries:
            problems.append(f"{path.name}: no [root] or [[crate]] entry")
        for target, entry in entries:
            label = "[root]" if target == ROOT_TARGET else f"[[crate]] {target or '?'}"
            if target != ROOT_TARGET and target not in crates:
                problems.append(f"{path.name}: {label}: not a published crate")
            if entry.get("section") not in SECTIONS:
                problems.append(f"{path.name}: {label}: section must be one of {', '.join(SECTIONS)}")
            if target == ROOT_TARGET and not entry.get("area", "").strip():
                problems.append(f"{path.name}: {label}: area is required")
            text = entry.get("text", "")
            if not text.strip():
                problems.append(f"{path.name}: {label}: text is empty")
            if "\n" in text or "\n" in entry.get("area", ""):
                problems.append(f"{path.name}: {label}: text is one line")
            if "unreleased" in (text + entry.get("area", "")).lower():
                problems.append(f"{path.name}: {label}: fragments never name [Unreleased]")
        fragments.append((fid, entries))
    seen = {}
    for fid, _ in fragments:
        seen[fid] = seen.get(fid, 0) + 1
    problems += [f"{fid}: id used by {count} fragments" for fid, count in seen.items() if count > 1]
    if problems:
        raise FragmentError("\n".join(problems))
    return fragments


def bullet(target, entry):
    if target != ROOT_TARGET:
        return f"- {entry['text']}"
    prefix = "**Breaking:** " if entry["section"] == "Breaking" else ""
    return f"- {prefix}**{entry['area']}:** {entry['text']}"


def render_blocks(fragments, version, date):
    """Changelog-relative path -> the block text for that changelog."""
    collected = {}
    for fid, entries in sorted(fragments, key=lambda f: f[0]):
        for target, entry in entries:
            collected.setdefault(target, {}).setdefault(entry["section"], []).append(bullet(target, entry))
    blocks = {}
    for target, by_section in collected.items():
        lines = [f"## [{version}] - {date}", ""]
        for section in SECTIONS:
            if section in by_section:
                lines += [f"### {section}", "", *by_section[section], ""]
        blocks[target] = "\n".join(lines) + "\n"
    return blocks


def changelog_path(root, crates, target):
    base = root if target == ROOT_TARGET else root / crates[target]
    return base / "CHANGELOG.md"


def insert_block(existing, block, version):
    if re.search(rf"^##\s+\[?{re.escape(version)}\]?(\s|$)", existing, re.M):
        raise FragmentError(f"already carries a heading for {version}")
    match = re.search(r"^## ", existing, re.M)
    if match:
        return existing[:match.start()] + block + existing[match.start():]
    head = existing.rstrip("\n") + "\n\n" if existing.strip() else "# Changelog\n\n"
    return head + block.rstrip("\n") + "\n"


def assemble(root, version, date, dry_run, out=None):
    out = out or sys.stdout
    crates = published_crates(root)
    fragments = load_fragments(root, crates)
    if not fragments:
        raise FragmentError(f"no fragments in {FRAGMENT_DIR}/ to assemble")
    blocks = render_blocks(fragments, version, date)
    order = sorted(blocks, key=lambda t: (t != ROOT_TARGET, str(changelog_path(root, crates, t))))
    writes = []
    for target in order:
        path = changelog_path(root, crates, target)
        existing = path.read_text() if path.is_file() else ""
        try:
            writes.append((path, insert_block(existing, blocks[target], version)))
        except FragmentError as error:
            raise FragmentError(f"{path.relative_to(root)}: {error}") from error
    if dry_run:
        for target in order:
            out.write(f"==> {changelog_path(root, crates, target).relative_to(root)}\n{blocks[target]}\n")
        return 0
    for path, content in writes:
        path.write_text(content)
    for fid, _ in fragments:
        (root / FRAGMENT_DIR / f"{fid}.toml").unlink()
    out.write(f"changelog-assemble: {len(fragments)} fragment(s) written to {len(writes)} changelog(s)\n")
    return 0


def check(root, out=None):
    out = out or sys.stdout
    fragments = load_fragments(root, published_crates(root))
    out.write(f"changelog-assemble: {len(fragments)} fragment(s) valid\n")
    return 0


def named_crates(root, out=None):
    out = out or sys.stdout
    fragments = load_fragments(root, published_crates(root))
    names = {target for _, entries in fragments for target, _ in entries if target != ROOT_TARGET}
    out.write("".join(f"{name}\n" for name in sorted(names)))
    return 0


def main(argv=None, root=ROOT):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--named-crates", action="store_true")
    parser.add_argument("--version")
    parser.add_argument("--date")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args(argv)
    try:
        if args.check:
            if args.version or args.date or args.dry_run:
                parser.error("--check takes no other argument")
            return check(root)
        if args.named_crates:
            if args.version or args.date or args.dry_run:
                parser.error("--named-crates takes no other argument")
            return named_crates(root)
        if not (args.version and args.date):
            parser.error("give --check, or --version and --date")
        if not VERSION_RE.match(args.version) or not DATE_RE.match(args.date):
            parser.error("--version is X.Y.Z and --date is YYYY-MM-DD")
        return assemble(root, args.version, args.date, args.dry_run)
    except FragmentError as error:
        print(f"changelog-assemble: {error}", file=sys.stderr)
        return 1
    except subprocess.CalledProcessError as error:
        print(f"changelog-assemble: cannot list manifests: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
