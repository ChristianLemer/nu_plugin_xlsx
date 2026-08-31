# Claude notes for nu_plugin_xlsx

Project-specific instructions for AI assistants working in this repo.

## VCS: Jujutsu (jj), not plain git

This repo is driven by **jj**. A clone may be colocated (`.git` and `.jj` side by side) or not (`.jj/` alone, the backing git repo inside `.jj/repo/store/git/`) — `jj git clone --colocate` decides it per machine, so check before assuming.

- Use `jj` commands for local VCS operations (`jj st`, `jj describe`, `jj new`, `jj bookmark`, `jj git push`).
- **Never read repository state from git.** Non-colocated, `git status` fails outright. Colocated, it answers — and the answer misleads: always *detached HEAD*, because jj keeps HEAD detached and drives the working copy itself. A succeeding `git status` is the dangerous case, since it reads like a normal git repo.
- Default bookmark is `trunk` (matches the GitHub default branch). Verify with `jj bookmark list` before advancing.
- Tags: jj doesn't manage tags natively. Push the bookmark with `jj git push`, then create the tag on GitHub (`gh release create vX.Y.Z ...`).
- `gh` CLI works normally — only the working-copy interaction differs from standard git.

## Release hygiene

### The version string carries two facts

A version names **this project's maturity** *and* **which Nushell the binary loads into**:

```
0.2.1+nu-0.114.1
└─┬──┘└────┬────┘
  │        └── build metadata: the Nushell minor this binary targets
  └── our own semver, monotonic, never encodes Nushell
```

Why the target must be stated at all: the plugin protocol version is a compile-time constant and Nushell rejects anything outside a caret match, so **every Nushell minor is a hard break** and no binary serves two. One release per Nushell minor. The reasoning is in [SPEC.md](SPEC.md#nushell-version-compatibility).

Never let the Nushell version into the semver itself. `0.2.1` → `0.2.2` means our code moved; `+nu-0.114.1` → `+nu-0.115.1` means the target moved. Collapsing them makes `jj log` unreadable — you could no longer tell a new release from the same code rebuilt.

### Three things that MUST agree before a release

1. `Cargo.toml` version → semver, no `v` prefix (e.g. `0.2.1+nu-0.114.1`)
2. Git tag / GitHub release → same string, `v` prefix (e.g. `v0.2.1+nu-0.114.1`)
3. `nu-plugin` / `nu-protocol` → pinned **exactly** (`=0.114.1`) to the version the metadata names

CI enforces all three in [.github/workflows/release.yml](.github/workflows/release.yml):

- the `test` job fails fast if `${GITHUB_REF_NAME#v}` doesn't equal the Cargo.toml version;
- [scripts/check-nu-metadata.sh](scripts/check-nu-metadata.sh) fails if the `+nu-` metadata doesn't equal the `nu-plugin` version `Cargo.lock` resolves to.

The second guard runs `--strict` at release and lax on CI — absent metadata only warns there, because an infra commit legitimately lands before the bump commit that restates the target.

**Commit convention for releases:**

A release cut is an **isolated "Bump" commit**: it changes the `Cargo.toml` version string, the two `nu-*` exact pins, and the cascading `Cargo.lock` — nothing else. Any other change — CI, docs, non-`nu` deps, CLAUDE.md — goes in its *own* commit that lands before the bump. The tag points at the pure-bump commit.

**The `nu-*` pins belong in the bump, not in an infra commit.** They are not a separate decision: `+nu-0.114.1` and `=0.114.1` state one fact in two places, and splitting them would leave a commit where the guard fails by construction.

Why the isolation: keeps `jj log` / `git log --grep "^Bump"` a clean timeline of every release, and lets you revert or cherry-pick a version bump without dragging unrelated changes along. Commit messages for bumps are short: `"Bump to 0.2.1+nu-0.114.1"`.

**Release sequence:**

1. Land all other changes first (CI tweaks, doc updates, non-`nu` dep bumps) as normal commits on `trunk`.
2. In a fresh working-copy change, edit `Cargo.toml` only: the version (e.g. `0.2.1+nu-0.114.1`) and the two `nu-*` pins to match (`=0.114.1`). Run `cargo check` once so `Cargo.lock` updates.
3. Verify locally before committing: `./scripts/check-nu-metadata.sh --strict`.
4. `jj describe -m "Bump to X.Y.Z+nu-A.B.C"` and `jj new`.
5. `jj bookmark move trunk --to @-` then `jj git push`.
6. Create the tag: `vX.Y.Z+nu-A.B.C` (on GitHub via `gh release create`, or inside the backing git repo). `+` is legal in a git ref name.
7. Watch Actions — both guards validate before building.

Supporting a new Nushell minor is this same sequence with a patch bump: `0.2.1+nu-0.114.1` → `0.2.2+nu-0.115.1`. The source should need no change; if it does, prefer fixing it in a way that keeps one source tree serving every supported minor (see [SPEC.md](SPEC.md#nushell-version-compatibility)) rather than adding version-gated code.

Pre-releases use `-beta.N` or `-rc.N`, and the suffix goes **before** the build metadata: `0.2.2-beta.1+nu-0.115.1`. Pick by intent — `-beta` when the release path or the packaging is what needs exercising, `-rc` when the code is believed final and only confirmation is missing. They are published to GitHub only, never to crates.io: cargo excludes pre-releases from normal resolution, so publishing them would add noise without helping anyone install, while a GitHub binary is exactly what a tester wants.

A pre-release is a real bump commit like any other — the version string changes, so `0.2.2-beta.1+nu-0.115.1` becoming `0.2.2+nu-0.115.1` is a second bump, not a re-tag.

The release workflow flags a prerelease by reading the **version core**, not the whole tag: it strips the build metadata at `+` and looks for a hyphen in what remains. That detail is not cosmetic — matching `-rc` alone would ship a `-beta` as stable, and matching any hyphen would mark *every* release as a prerelease, since `+nu-0.115.1` contains one. The current form accepts `-alpha`, `-beta` and `-rc` without further change.

## Design authority

- [SPEC.md](SPEC.md) is the authority for scope and command design. Consult it before expanding surface area.
- [README.md](README.md) is user-facing; keep it short and example-driven.

## Where a document lives

Decide by **mutability**, not importance.

- **Will be edited again** — plans, handovers, session state, brainstorm scaffolds. They live in the vault, reached through the `meta` and `docs` symlinks, and are never versioned. They are transit: superseded, then deleted.
- **Finished when written** — a decision and its reason. Versioned, in `SPEC.md`, in the same commit as the code it justifies.

Why mutability and not importance: history here gets rewritten. A document edited across many commits is dragged through every rebase and can land in a commit that predates the decision it records. A document written once beside its code moves with that code, untouched.

The test: if losing the vault entirely left an unanswerable "why is this code like this?", the split is wrong. Deliberation dies in the vault; the outcome lands in `SPEC.md`.

`## Open questions` in `SPEC.md` holds a question until it is settled. Settling it means moving it into the body of the spec, in the commit that implements it — never recording the answer in a plan.
