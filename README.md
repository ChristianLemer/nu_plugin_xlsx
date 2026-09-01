# nu_plugin_xlsx

A Nushell plugin for writing Excel (.xlsx) files. Outputs real Excel Table objects with auto-filter, banded rows, and autofit by default.

## Install

```nushell
http get https://raw.githubusercontent.com/ChristianLemer/nu_plugin_xlsx/HEAD/install.nu | save -f install.nu
nu install.nu --register
```

That is all. The installer reads the Nushell running it, picks the matching build,
verifies its checksum and registers it. No Rust toolchain.

Then add `plugin use xlsx` to your config, so the commands survive a restart —
`plugin add` only writes the registry, it doesn't load anything into scope.

> **Re-run the installer after every Nushell upgrade.** A plugin binary loads into
> exactly one Nushell minor: the protocol version is a compile-time constant, so
> every minor is a hard break and no binary serves two. Upgrading Nushell silently
> stops every plugin from loading.
>
> The failure names nothing useful — `plugin add` reports
> `nu::shell::io::broken_pipe` / `PluginWrite could not flush`, never a word about
> versions. If you see that, it is a version mismatch. The installer keeps a copy of
> itself beside the binary, so re-running is local:
>
> ```nushell
> nu ($nu.data-dir | path join plugins install.nu) --register
> ```

### Install from a release download

If you would rather do it by hand. Assets on
[Releases](https://github.com/ChristianLemer/nu_plugin_xlsx/releases) are named
`nu_plugin_xlsx-nu<nu-version>-<target>.tar.gz` (`.zip` on Windows), one per platform:

| Target | For |
| --- | --- |
| `x86_64-unknown-linux-musl` | any Linux — statically linked, no glibc requirement |
| `aarch64-apple-darwin` | Apple Silicon |
| `x86_64-apple-darwin` | Intel Mac |
| `x86_64-pc-windows-msvc` | Windows |

Pick the one whose `nu<nu-version>` matches yours — check with `version | get version`.

Extract it — the binary inside is already named `nu_plugin_xlsx`, which matters because Nushell
refuses to register a file whose name doesn't start with `nu_plugin_`:

```nushell
tar xzf nu_plugin_xlsx-nu0.115.1-x86_64-unknown-linux-musl.tar.gz
```

On macOS, clear the quarantine flag:

```nushell
xattr -d com.apple.quarantine nu_plugin_xlsx
```

Then register it — `plugin add` records the full path, so put the binary where it will stay:

```nushell
plugin add ./nu_plugin_xlsx
plugin use xlsx
```

Each asset ships a `.sha256` beside it if you want to verify the download.

### Install from crates.io

Only if you have Rust and want to build against your own Nushell.

```nushell
cargo install nu_plugin_xlsx --locked
```

⚠️ **This picks the wrong build more often than not.** The Nushell target lives in the
version's build metadata (`0.2.3+nu-0.115.1`), and semver requires build metadata to be
*ignored* during resolution — so cargo always takes the newest release, whichever Nushell
it targets. On Nushell 0.113 you would get the 0.115 build, which cannot load.

To build from source for your own Nushell, check out the tag whose `+nu-` matches it
and `cargo install --path .` from there.

## Usage

```nushell
# Simplest — save detects the .xlsx extension and calls `to xlsx` for you
ls | save files.xlsx

# Explicit conversion (also works — binary input is passed through)
ls | to xlsx | save files.xlsx

# Multi-sheet workbook
{ Users: $users, Orders: $orders } | save report.xlsx

# Plain cells (no Excel Table formatting)
ls | to xlsx --raw | save files.xlsx
```

> **Note:** Nushell's `save` command automatically invokes `to xlsx` when the file extension is `.xlsx`. You don't need to call `to xlsx` explicitly unless you want to use flags like `--raw` or pipe the binary elsewhere (e.g. `to xlsx | http post`).

### Reading back xlsx files

When reading back with the core `from xlsx`, the header row is returned as a data row. Pipe through `headers` to promote it:

```nushell
open report.xlsx | get Sheet1 | headers
```

## Test

```bash
cargo test
cargo clippy
```

Tests use `calamine` (dev dependency) to read back the generated xlsx and verify:

| Test                                  | What it checks                                         |
| ------------------------------------- | ------------------------------------------------------ |
| `single_table_creates_sheet1`         | Table input wraps as Sheet1                            |
| `multi_sheet_from_record`             | Record of tables creates named sheets                  |
| `empty_table_creates_empty_sheet`     | Empty input produces a valid empty sheet               |
| `raw_mode_no_table`                   | `--raw` flag skips Excel Table formatting              |
| `type_mapping_values`                 | String, int, float, bool, nothing round-trip correctly |
| `date_written_as_excel_date`          | Dates are written as real Excel dates                  |
| `sparse_records_with_missing_columns` | Missing columns produce empty cells                    |
| `mixed_types_in_same_column`          | Mixed types in a column are handled correctly          |
| `rejects_non_table_input`             | Non-table input returns an error                       |

## About

This is my first open-source project. I've been a software engineer in enterprise environments for years but hadn't published anything publicly until now.

This project is developed with [Claude Code](https://claude.ai/claude-code) as a pair programming partner. Design decisions, architecture, and code review are human-driven — AI assists with implementation, research, and iteration.

## License

MIT
