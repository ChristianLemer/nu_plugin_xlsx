# Claude notes for nu_plugin_xlsx

Project-specific instructions for AI assistants working in this repo.

## Language: the repository is English

**Everything the repository carries is written in English.** `README.md`, `SPEC.md`, this
file, the code and its comments, commit messages, branch names, pull request titles and
bodies, and any reply to an issue. The project is public and its audience is the Nushell
community, which works in English.

The maintainer and the assistant often work in French. That belongs to the conversation and
to the notes folder, and it stops at the commit boundary. Write the English directly rather
than drafting in French and translating: a translated commit message reads like one, and the
reasoning is the whole point of these messages.

## VCS: plain git, one line of history

Plain git, driven from git worktrees, one per piece of work. No jj: an earlier incarnation of
this repo used it, and any jj trace you meet is stale.

- `trunk` is the only long-lived branch and matches the GitHub default. It moves only by merging
  a pull request whose checks are green. Never commit to it directly, never force-push it.
- Every change, the maintainer's included, goes on a short branch named for the work
  (`installer-checksum`, not `check-status-latest-commit`), gets a pull request, merges with a
  **rebase** so the line stays straight, and the branch is deleted at merge. Squash only when the
  commits carry no reasoning worth keeping — here they usually do.
- No stale branches on the remote. A branch that is merged, or whose commit a tag already holds,
  is deleted. Visitors read the branch list as the state of the project.
- Tags are the releases, and they point at commits that hang off `trunk` — see *Release hygiene*.
- **Rehearse the release without publishing:** `gh workflow run release.yml --ref <branch>` runs
  the whole matrix, packages, installs and loads on all four hosts, and creates nothing. Do it
  before merging anything that touches the workflows or the installer.
- `gh` is the tool for anything on GitHub: runs, pull requests, releases.

**How a change gets reviewed.**

- **Review before the first push.** Run the code-review skill on the branch in a fresh context,
  and the security review too for anything touching `install.nu` or the workflows — those run on
  other people's machines. **Check every finding in a running shell before applying it.** An AI
  reviewer is confidently wrong often enough to matter: of the first three findings raised here,
  one was invalid, and applying it would have deleted a correct line. Fix what survives, tidy the
  commits, then push once. The pull request then shows the commits and green checks, `ci` being
  the one trunk requires.
- **The order is review, push, rehearse, merge.** A second push is for what only the remote can
  tell you: a CI job red on macOS or Windows, or a red release rehearsal — `gh workflow run`
  resolves the workflow file server-side, so the branch must be pushed before it can run.
  Nothing else earns one.
- **Say what the review changed, not what it said.** The description is for whoever reads the
  pull request, not a transcript: a finding you declined and why, and any question it settled.
  A finding that settles a design question goes into `SPEC.md` as well, since a pull request
  body is not versioned and a rebase merge leaves no commit carrying it.
- **The review bot is off, not gone.** `.coderabbit.yaml` disables its automatic review, so it
  says nothing unless asked. Summon it with `@coderabbitai review` on a pull request touching
  `install.nu`, the workflows, or anything else a colleague runs: it is a different system with
  different blind spots, and on the first pull request here it caught two things the in-session
  review had missed. Left automatic it was worse than useless — one review an hour on the free
  plan, so it skipped the bursts where a second look matters, and four comments per finding.

## Setting up on a new machine

None of this travels with the clone, and each item below has cost time at least once.

**Toolchain.** Rust stable, edition 2021, plus a Nushell whose minor matches the `+nu-`
metadata you intend to build against. No system packages are needed: the dependency tree
carries no C library, so a bare `cargo build` suffices.

**The `meta` and `docs` symlinks.** The maintainer keeps plans, notes and specs in a folder
outside the repository, synced between machines. `meta` points at that folder, `docs` at its
`docs/` subdirectory. Both are absolute, so the path differs per machine. Set `V` to it, then
check the links resolve — `ln -s` succeeds happily on a path that does not exist, and the
failure only surfaces much later:

```bash
V="$HOME/…"        # the project's notes folder on this machine
ln -s "$V" meta && ln -s "$V/docs" docs
ls meta/ docs/     # both must list, or the links are dangling
```

Both are ignored in `.gitignore`, so `git add -A` never picks them up and a checkout never
touches them. See the comment there for why the entries carry no trailing slash: with one, a
symlink stops matching, and the next `git add -A` would commit a path that exists on one
machine only.

**Registering is not loading.** `plugin add` records signatures in the registry; `plugin use
xlsx` brings the commands into scope, and only for the current session. Put `plugin use xlsx`
in the config or the commands do not survive a restart.

**Build in debug to test, never `--release`.** The release profile combines LTO with
`codegen-units = 1`: 18 min 35 cold, against 2 min 30 in debug and under a second incremental,
for a functionally identical binary. Release builds are the CI's job.

**A registered plugin shadows a fresh binary.** `nu --plugins <path>` does *not* win over an
already-registered plugin of the same name — the registered one answers, and your new build is
never exercised. To test a fresh build, add a throwaway registry:

```bash
nu --plugin-config /tmp/scratch.msgpackz --plugins ./target/debug/nu_plugin_xlsx
```

**A version mismatch says its name only on one route.** Registering a plugin built against the
wrong Nushell minor with `plugin add` fails as `Failed to send plugin call` or
`nu::shell::io::broken_pipe`, without one word about versions. Loading it with `nu --plugins`
instead names both versions. When a registration fails, compare `nu --version` against the
`+nu-` metadata in `Cargo.toml`, or load with `--plugins` and read the message.

**Test against every supported minor, not just the one on PATH.** The unit tests never start a
Nushell, so they cannot tell whether a binary loads. `scripts/test-nu-compat.sh` can: for each
version in `supported-nu.txt` it copies the tree under `target/nu-compat/`, repins the two
`nu-*` crates there, builds in debug, and runs `scripts/smoke.nu` inside that exact Nushell with
a throwaway registry. The working copy is never touched. The shells come from mise, side by
side, and only the default is on PATH:

```bash
mise install aqua:nushell/nushell@0.113.1 aqua:nushell/nushell@0.114.1
./scripts/test-nu-compat.sh --all        # or one: ./scripts/test-nu-compat.sh 0.114.1
```

CI runs the same script per minor on every push (`nu-compat` in `ci.yml`), and the release
workflow runs `install.nu` plus the smoke on every packaged archive before uploading it. To
add or drop a minor, edit `supported-nu.txt` and nothing else.

**Run the CI gates before committing.** `cargo fmt -- --check` is a gate, not a suggestion, and
it is the one that gets forgotten; clippy runs stricter in CI than a bare `cargo clippy` does
locally. The four, in the form CI runs them — the last one needs the mise shells above:

```bash
cargo fmt -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
./scripts/test-nu-compat.sh --all
```

**One session per worktree.** Two agent sessions in the same worktree overwrite each other's
working copy. A second session gets its own `git worktree add`.

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

Never let the Nushell version into the semver itself. `0.2.1` → `0.2.2` means our code moved; `+nu-0.114.1` → `+nu-0.115.1` means the target moved. Collapsing them makes `git log` unreadable — you could no longer tell a new release from the same code rebuilt.

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

Why the isolation: keeps `git log --grep "^Bump"` a clean timeline of every release, and lets you revert or cherry-pick a version bump without dragging unrelated changes along. Commit messages for bumps are short: `"Bump to 0.2.1+nu-0.114.1"`.

**Release sequence:**

1. Land all other changes first (CI tweaks, doc updates, non-`nu` dep bumps) through pull requests on `trunk`. The commit `trunk` then sits on is the release point.
2. Branch off it for one variant: `git switch -c bump/X.Y.Z+nu-A.B.C trunk`. Edit `Cargo.toml` only: the version (e.g. `0.2.1+nu-0.114.1`) and the two `nu-*` pins to match (`=0.114.1`). Run `cargo check` once so `Cargo.lock` updates.
3. Verify before committing: `./scripts/check-nu-metadata.sh --strict`.
4. `git commit -am "Bump to X.Y.Z+nu-A.B.C"`.
5. Tag the bump commit and push the tag — **not** `gh release create`, and not the branch:

```bash
git tag "vX.Y.Z+nu-A.B.C"
git push origin "vX.Y.Z+nu-A.B.C"
git switch trunk && git branch -D "bump/X.Y.Z+nu-A.B.C"   # the tag holds the commit
```

Repeat 2–5 from the same release point for each minor in `supported-nu.txt`. `trunk` never moves during a release; the variants hang off it, and only the tags reach the remote.

`+` is legal in a git ref name. Three reasons the tag push wins:

- **The release workflow creates the release itself**, assets attached, on `push: tags: ["v*"]`. `gh release create` would publish an empty release first and let the action fill it in afterwards — a window in which a public release has no binaries.
- **The target is resolved locally.** `gh release create --target <branch>` resolves server-side, so it tags whatever the remote currently knows the branch to be — which, after a rewrite, may be the old stack. A local tag names the commit object and can be checked before it leaves.
- **A local tag is free to delete.** Nothing is public until `git push origin <tag>`.
6. Watch Actions — both guards validate before building.

**A tag push fires two workflows, not one.** `ci.yml` triggers on every push, tags included, so
each tag produces a `Release` run *and* a `CI` run. Watching only the release runs can report
green while `fmt`/`clippy`/`test` is red on the same tag — and watching only by run id, as
`gh run watch` does, silently omits the other. List by tag instead:

```bash
gh run list --branch "vX.Y.Z+nu-A.B.C"
```

### One release, one variant per supported Nushell minor

A release is not a single artifact. The same code is cut once per Nushell minor still supported, and those cuts **share a semver** — they differ only in build metadata, which is precisely what semver says build metadata is for: the same version, built differently.

```
0.2.1+nu-0.113.0
0.2.1+nu-0.114.1   ← one code, three targets, one version
0.2.1+nu-0.115.1
```

Do *not* number them `0.2.1`, `0.2.2`, `0.2.3`: that would assert three code changes that never happened, and the rule above says a semver step means our code moved.

Supporting a **new** Nushell minor when the source has not changed therefore adds a variant, not a version: `0.2.1+nu-0.116.0`. The source should need no change; if it does, prefer a fix that keeps one source tree serving every supported minor (see [SPEC.md](SPEC.md#nushell-version-compatibility)) over version-gated code. When the source *does* move, cut a new semver and re-issue the variants for the minors still supported.

**Shape in the log.** Variants are sibling leaf commits hanging off the release point, one commit each, tagged and never touched again. `trunk` stays on the release point, not on a variant. The code stays a single line — there are no long-lived per-Nushell branches to maintain.

**crates.io holds exactly one.** It resolves by semver and ignores build metadata, so it cannot carry `0.2.1` three times. Publish the newest target there, and treat crates.io as the last install route: a user on an older Nushell who runs `cargo install` gets a binary that cannot load. GitHub releases carry the full set, and `install.nu` picks from them by reading the running Nushell.

**How deep to support.** Roughly the current minor and the two before it. Distributions lag — Arch shipped 0.113 while 0.115 was current — and a shorter window leaves whole distributions with no usable binary. The actual list is `supported-nu.txt`: one exact version per minor, read by the compatibility script and by CI. Dropping a minor is a one-line commit there, and the commit message is where the reason goes.

Pre-releases use `-beta.N` or `-rc.N`, and the suffix goes **before** the build metadata: `0.2.2-beta.1+nu-0.115.1`. Pick by intent — `-beta` when the release path or the packaging is what needs exercising, `-rc` when the code is believed final and only confirmation is missing. They are published to GitHub only, never to crates.io: cargo excludes pre-releases from normal resolution, so publishing them would add noise without helping anyone install, while a GitHub binary is exactly what a tester wants.

A pre-release is a real bump commit like any other — the version string changes, so `0.2.2-beta.1+nu-0.115.1` becoming `0.2.2+nu-0.115.1` is a second bump, not a re-tag.

The release workflow flags a prerelease by reading the **version core**, not the whole tag: it strips the build metadata at `+` and looks for a hyphen in what remains. That detail is not cosmetic — matching `-rc` alone would ship a `-beta` as stable, and matching any hyphen would mark *every* release as a prerelease, since `+nu-0.115.1` contains one. The current form accepts `-alpha`, `-beta` and `-rc` without further change.

## Design authority

- [SPEC.md](SPEC.md) is the authority for scope and command design. Consult it before expanding surface area.
- [README.md](README.md) is user-facing; keep it short and example-driven.

## Where a document lives

Decide by **mutability**, not importance.

- **Will be edited again** — plans, handovers, session state, brainstorm scaffolds. They live in the notes folder, reached through the `meta` and `docs` symlinks, and are never versioned. They are transit: superseded, then deleted.
- **Finished when written** — a decision and its reason. Versioned, in `SPEC.md`, in the same commit as the code it justifies.

Why mutability and not importance: branches here get rebased before they merge. A document edited across many commits is dragged through every rebase and can land in a commit that predates the decision it records. A document written once beside its code moves with that code, untouched.

The test: if losing the notes folder entirely left an unanswerable "why is this code like this?", the split is wrong. Deliberation dies in the notes; the outcome lands in `SPEC.md`.

`## Open questions` in `SPEC.md` holds a question until it is settled. Settling it means moving it into the body of the spec, in the commit that implements it — never recording the answer in a plan.
