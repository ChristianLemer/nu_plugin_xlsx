# nu_plugin_xlsx

A Nushell plugin for writing Excel (.xlsx) files. Outputs real Excel Table objects with auto-filter, banded rows, and autofit by default.

## Install

```nushell
cargo install nu_plugin_xlsx --locked
plugin add (which nu_plugin_xlsx | get path.0)
plugin use xlsx
```

> **Match your Nushell version.** A plugin binary loads into exactly one Nushell
> minor — the protocol breaks on every one. Each release states its target in
> the version's build metadata, e.g. `0.2.2+nu-0.115.1` for Nushell 0.115.
> Check yours with `version | get version` and pick the matching release.
>
> A mismatch gives you no useful clue — `plugin add` fails with an opaque
> `nu::shell::io::broken_pipe` / `PluginWrite could not flush`, never a word
> about versions. If you see that, check the version first.

Add `plugin use xlsx` to your config so the commands survive a restart —
`plugin add` only writes the registry, it doesn't load anything into scope.

### Install from a release download

No Rust toolchain needed. Assets on [Releases](https://github.com/ChristianLemer/nu_plugin_xlsx/releases)
are named `nu_plugin_xlsx-nu<nu-version>-<target>.tar.gz` (`.zip` on Windows), one per platform:

| Target | For |
| --- | --- |
| `x86_64-unknown-linux-musl` | any Linux — statically linked, no glibc requirement |
| `aarch64-apple-darwin` | Apple Silicon |
| `x86_64-apple-darwin` | Intel Mac |
| `x86_64-pc-windows-msvc` | Windows |

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
