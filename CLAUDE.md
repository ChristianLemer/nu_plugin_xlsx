# Claude notes for nu_plugin_xlsx

Project-specific instructions for AI assistants working in this repo.

## VCS: Jujutsu (jj), not plain git

This repo uses **jj in non-colocated mode**: `.jj/` at the root, no `.git/` beside it. The backing git repo lives inside `.jj/repo/store/git/`.

- Use `jj` commands for local VCS operations (`jj st`, `jj describe`, `jj new`, `jj bookmark`, `jj git push`). Plain `git status` / `git commit` at the working-copy root will fail with "not a git repository".
- Default bookmark is `trunk` (matches the GitHub default branch). Verify with `jj bookmark list` before advancing.
- Tags: jj doesn't manage tags natively. Push the bookmark with `jj git push`, then create the tag on GitHub (`gh release create vX.Y.Z ...`).
- `gh` CLI works normally — only the working-copy interaction differs from standard git.

## Release hygiene

The version string lives in two places and they MUST match before a release:

1. `Cargo.toml` → semver, no `v` prefix (e.g. `0.1.0-rc.3`)
2. Git tag / GitHub release → `v` prefix (e.g. `v0.1.0-rc.3`)

CI enforces this in [.github/workflows/release.yml](.github/workflows/release.yml) — the `test` job fails fast if `${GITHUB_REF_NAME#v}` doesn't equal the Cargo.toml version.

**Commit convention for releases:**

A release cut is an **isolated "Bump" commit**: its only change is the `Cargo.toml` version string (and the cascading `Cargo.lock`). Any infra change — CI, docs, deps, CLAUDE.md — goes in its *own* commit that lands before the bump. The tag points at the pure-bump commit.

Why: keeps `jj log` / `git log --grep "^Bump"` a clean timeline of every release, and lets you revert or cherry-pick a version bump without dragging unrelated changes along. Commit messages for bumps are short: `"Bump to 0.1.0-rc.3"`.

**Release sequence:**

1. Land all infra changes first (CI tweaks, doc updates, dep bumps) as normal commits on `trunk`.
2. In a fresh working-copy change, edit only `Cargo.toml` to the target version (e.g. `0.1.0-rc.3` or `0.1.0`). `cargo build` or `cargo check` once so `Cargo.lock` updates.
3. `jj describe -m "Bump to X.Y.Z"` and `jj new`.
4. `jj bookmark move trunk --to @-` then `jj git push`.
5. Create the tag: `vX.Y.Z` (on GitHub via `gh release create`, or inside the backing git repo).
6. Watch Actions — the guard validates the match before building.

Pre-releases use `-rc.N` suffixes; the release workflow marks them as prereleases automatically when `-rc` is in the tag.

## Design authority

- [SPEC.md](SPEC.md) is the authority for scope and command design. Consult it before expanding surface area.
- [README.md](README.md) is user-facing; keep it short and example-driven.
