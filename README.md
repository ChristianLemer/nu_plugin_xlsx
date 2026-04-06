# nu_plugin_xlsx

A Nushell plugin for writing Excel (.xlsx) files. Outputs real Excel Table objects with auto-filter, banded rows, and autofit by default.

> **Status**: Work in progress — not ready for use or contributions yet. See [SPEC.md](SPEC.md) for the design.

## Build

```bash
cargo build --release
```

## Install

```bash
plugin add target/release/nu_plugin_xlsx
plugin use xlsx
```

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
