# Changelog fragments

Every change that a changelog should mention lands as one file here,
`changelog.d/<id>.toml`. At release, `scripts/release.sh` runs
`scripts/changelog-assemble.py --version X.Y.Z --date YYYY-MM-DD`, which writes
the `## [X.Y.Z] - date` block into the root `CHANGELOG.md` and into each named
crate's `CHANGELOG.md`, then deletes the fragments it consumed. Nobody edits a
`CHANGELOG.md` by hand between releases.

## Format

```toml
[root]
section = "Breaking"
area = "Extensions"
text = "`ExtensionRouter::new` takes an `AuthzPolicy`. Migrate by ..."

[[crate]]
name = "systemprompt-extension"
section = "Breaking"
text = "`ExtensionRouter::new` takes an `AuthzPolicy` and a `ContextKind`. Migrate by ..."
```

- `[root]` (at most one): the entry in the root `CHANGELOG.md`. `area` is the
  bold label (`**Extensions:**`); a `Breaking` entry is also prefixed
  `**Breaking:**`.
- `[[crate]]` (any number, a crate may repeat): a plain bullet in that crate's
  `CHANGELOG.md`. `name` is the package name of a published crate.
- `section` is one of `Breaking`, `Added`, `Changed`, `Fixed`, `Removed`.
- Values are one-line double-quoted strings; `\"`, `\\` and `\uXXXX` escape.
  Text is written verbatim after the bullet, so it follows the changelog
  register: the current behaviour, not its history, and never `[Unreleased]`.
- `<id>` is lowercase letters, digits and dashes. Within a section, entries are
  ordered by fragment id, then by their order in the fragment.

## Gates

- `just changelog-check` (`changelog-assemble.py --check`) validates every
  fragment; it is a source gate.
- `just check-crate-changelogs` accepts a crate whose `src` changed since the
  last release tag when a fragment names it, or, once the release bump has
  assembled them, when its `CHANGELOG.md` carries the version heading.
- `scripts/changelog-assemble.py --dry-run --version X.Y.Z --date YYYY-MM-DD`
  prints the blocks without writing anything.
