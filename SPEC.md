# nu_plugin_xlsx — Design Spec

## Purpose

A Nushell plugin for writing Excel (.xlsx) files.
Fills a gap in Nushell's ecosystem: `from xlsx` exists in core (reading), but there is no `to xlsx` (writing).

## Scope

### v0.1 — Write

Ship `to xlsx` first. This is the missing capability.

### Future — Read (conditional)

If [nushell#16711](https://github.com/nushell/nushell/issues/16711) lands and `from xlsx` is removed from core, this plugin may absorb it. Until then, `from xlsx` is out of scope.

## Dependencies

| Crate                                                    | Version   | Purpose                              |
| -------------------------------------------------------- | --------- | ------------------------------------ |
| `nu-plugin`                                              | `0.111.0` | Nushell plugin protocol              |
| `nu-protocol`                                            | `0.111.0` | Nushell types (`Value`, `Span`, etc) |
| `rust_xlsxwriter` (features: `chrono`, `ryu`)            |           | Write .xlsx files                    |
| `chrono`                                                 |           | Date/time mapping                    |

## Command: `to xlsx`

### Input model

- **Record of tables** (keys = sheet names, values = tables) → multi-sheet workbook. This is the canonical input and the inverse of what `from xlsx` returns.
- **Table** (list of records) → syntactic sugar, wrapped internally as `{ Sheet1: $table }`.

### Output model

Emits **binary data** to the pipeline. Follows Nushell convention for `to *` commands.

By default, each sheet is written as a real Excel Table object (auto-filter, banded rows, structured references).

```nushell
# Single sheet — sugar
ls | to xlsx | save files.xlsx

# Multi-sheet — canonical form
{ Users: $users, Orders: $orders } | to xlsx | save report.xlsx
```

### Flags

| Flag           | Type | Default | Description                                  |
| -------------- | ---- | ------- | -------------------------------------------- |
| `--raw` / `-r` | bool | `false` | Write plain cells instead of an Excel Table  |

### Type mapping (Nushell → Excel)

| Nushell type         | Excel cell type | Notes                              |
| -------------------- | --------------- | ---------------------------------- |
| `string`             | Text            |                                    |
| `int`                | Integer         |                                    |
| `float`              | Float           |                                    |
| `bool`               | Boolean         |                                    |
| `date` / `datetime`  | Date            | Format: `yyyy-mm-dd hh:mm:ss`      |
| `duration`           | Number          | Seconds as float                   |
| `filesize`           | Number          | Bytes as integer                   |
| `nothing` / `null`   | Empty cell      |                                    |
| `list` / `record`    | String          | Expanded string representation     |

## Command: `from xlsx` (conditional)

Contingent on [nushell#16711](https://github.com/nushell/nushell/issues/16711). If `from xlsx` is removed from core, this plugin would mirror the existing command signature. Returns a record where keys are sheet names and values are tables.

```nushell
open --raw report.xlsx | from xlsx
```

### Flags

| Flag               | Type            | Default | Description                       |
| ------------------ | --------------- | ------- | --------------------------------- |
| `--sheets` / `-s`  | list\<string\>  | all     | Which sheets to read              |
| `--no-header`      | bool            | `false` | Don't treat first row as headers  |
| `--no-infer`       | bool            | `false` | Return all values as strings      |

## Project structure

```text
nu_plugin_xlsx/
├── Cargo.toml
├── LICENSE
├── README.md
├── SPEC.md
├── src/
│   ├── main.rs          # Entry point: serve_plugin()
│   ├── lib.rs           # Plugin struct, registers commands
│   └── to_xlsx.rs       # `to xlsx` command + inline tests
```

## Build & install

```bash
cargo build --release
plugin add target/release/nu_plugin_xlsx
plugin use xlsx
```

## Design principles

1. **Idiomatic Nushell.** Follow the conventions of `to csv`, `to json`, `from xlsx`. Emit binary, accept pipeline input, use standard flags.
2. **Correct by default.** Type mapping should just work without flags. A bare `ls | to xlsx | save files.xlsx` should produce a well-formatted spreadsheet.
3. **Progressive formatting.** Zero-config output is good. Flags unlock better output. A future config-record system unlocks full `rust_xlsxwriter` power.
4. **Symmetry.** `from xlsx` and `to xlsx` should be inverses: `data | to xlsx | from xlsx` should round-trip cleanly.

## Implementation notes

- **Plugin trait**: `PluginCommand` with `PipelineData` — collects input via `into_value()`.
- **Binary output**: `Value::binary(bytes, span)`.
- **Binary passthrough**: If input is already binary, pass it through (handles `to xlsx | save foo.xlsx`).
- **Input/output types**: Two variants — `Type::table()` (sugar) and `Type::Record` (multi-sheet). Both produce `Type::Binary`.
- **Error handling**: `LabeledError` with `Span` from the source value.
- **Lints**: `clippy::pedantic`, deny `unsafe_code` and `unwrap_used`.
- **Edition**: Rust 2021 (matches `nu-plugin`).

## Open questions

- [ ] Which `rust_xlsxwriter` Table style to use as default?
