# nu_plugin_xlsx — Design Spec

## Purpose

A Nushell plugin for writing Excel (.xlsx) files.
`from xlsx` exists in core (reading); core has no `to xlsx` (writing).

### Prior art

One other plugin covers this ground: [`nu_plugin_to_xlsx`](https://github.com/eggcaker/nu_plugin_to_xlsx)
(v0.5.0, MIT). It declares the same command name, `to xlsx`, so registering both conflicts.

| | `nu_plugin_to_xlsx` 0.5.0 | this plugin |
| --- | --- | --- |
| Output | writes the file itself — `path` is a required argument, returns `Nothing` | emits binary into the pipeline |
| Excel Table | plain cells only | real Table object, auto-filter, banded rows, autofit |
| Nushell target | `nu-plugin ^0.110.0` | pinned to the release's `+nu-` metadata |
| Tests | none | round-trip via `calamine` |

The output model is the substantive difference, and it is a design choice rather than a gap.
`ls | to xlsx Files.xlsx` has an effect and returns nothing; `ls | to xlsx | save files.xlsx` is
composable — the binary can go to `http post`, be hashed, or be streamed. That is the
`to csv` / `to json` convention, and Design principle 1 below.

## Scope

### v0.1 — Write

Ship `to xlsx` first. This is the missing capability.

### Future — Read (conditional)

If [nushell#16711](https://github.com/nushell/nushell/issues/16711) lands and `from xlsx` is removed from core, this plugin may absorb it. Until then, `from xlsx` is out of scope.

## Dependencies

| Crate                                          | Version           | Purpose                              |
| ---------------------------------------------- | ----------------- | ------------------------------------ |
| `nu-plugin`                                    | pinned exactly    | Nushell plugin protocol              |
| `nu-protocol`                                  | pinned exactly    | Nushell types (`Value`, `Span`, etc) |
| `rust_xlsxwriter` (features: `chrono`, `ryu`)  | `0.82`            | Write .xlsx files                    |
| `chrono`                                       | `0.4`             | Date/time mapping                    |

The two `nu-*` crates are pinned to an exact version (`=0.115.1`, not `0.115`)
because that version *is* the plugin's compatibility contract — see
[Nushell version compatibility](#nushell-version-compatibility). `Cargo.toml`
is the authority for which one; this table deliberately does not restate it.

## Nushell version compatibility

A plugin binary loads into exactly one Nushell minor. This is not a policy
choice, it is the protocol:

- `nu-plugin-protocol` sets its handshake version from `CARGO_PKG_VERSION` —
  a compile-time constant, not a negotiation.
- `ProtocolInfo::is_compatible_with` caret-matches the higher version against
  the lower. In `0.x`, `^0.114.1` means `>=0.114.1, <0.115.0`.

So a binary built against one minor is refused by another, and no single binary
can serve two.

The refusal is silent about its cause, which is why the version has to be
legible from the outside. Loading a 0.115-built binary into Nushell 0.113 gives
only `nu::shell::io::broken_pipe` / `PluginWrite could not flush` — the plugin
rejects the handshake and exits, and the shell reports the dead pipe. Nothing
mentions a version. (Verified in that direction; a newer shell against an older
plugin may report differently.)

**Consequence for releases.** The plugin's own semver tracks *this* project;
the Nushell target rides in build metadata: `0.2.1+nu-0.114.1`. One release per
Nushell minor, each pinning `nu-plugin` and `nu-protocol` exactly to the
version its metadata names. `scripts/check-nu-metadata.sh` enforces the match
mechanically, so the claim cannot rot. Release mechanics live in
[CLAUDE.md](CLAUDE.md).

**Consequence for source.** A single source tree serves every supported minor —
there is no version-gated code, and adding any would be a regression. Prefer
the constructor helpers (`Type::record()`, `Type::table()`) over raw enum
variants: the helpers keep their signature across minors, the variants do not.
`Type::Record`'s payload changed from `Box<[(String, Type)]>` to
`CollectionColumns<Type>` in 0.114, and `Type::record()` absorbed it.

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

### Empty input

**Current policy: an empty input produces a valid, entirely empty sheet — no
header row, no Excel Table.** This is a deliberate choice, not a fallback; it is
covered by `empty_table_creates_empty_sheet`.

It looks like a limitation because users ask for the header row to survive. It
cannot, and the reason is worth recording — three constraints, each verified
rather than assumed:

1. **Nushell cannot represent a header-only table.** `Value::List` carries only
   its rows; there is no column metadata on the value at any version from
   0.113 to 0.115. `Value::get_type()` *derives* `Type::Table(columns)` by
   inspecting the rows' records, so zero rows yields `list<any>` and the
   columns are gone. `[[a b]; [1 2]] | where a > 99 | columns` returns `[]`.
   The schema is destroyed upstream, before the plugin is reached — no plugin
   API recovers it (`EvaluatedCall` carries a span and arguments, nothing more).
   Note the trap: `[[a b];]` does not parse as an empty table, it parses as
   `list<list<string>>`, i.e. one data row.

2. **Excel forbids a header-only Table.** A `ListObject` with a header row must
   span at least two rows. `rust_xlsxwriter` refuses it (`Table must have at
   least one row`), and that mirrors Excel rather than being cautious: a
   hand-built `ref="A1:B1"` with `headerRowCount=1` opens *Repaired*, with
   `Removed Feature: Table` and `Removed Feature: AutoFilter`. The header cells
   survive as plain cells. Worth knowing: the minimum legal shape reserves the
   extra row in the table's `ref` without writing any cell for it — the sheet's
   `dimension` still stops at the header.

3. **A dataframe would not help.** `NuDataFrame` does carry a schema at zero
   rows, but it cannot cross a plugin boundary: `NuDataFrameCustomValue`
   marks its dataframe field `#[serde(skip)]`, so only a `Uuid` is transported
   and the data stays in the polars plugin's own process cache.

So the schema can only ever come from the caller. Accepting it is a surface
decision, not an implementation one — see [Open questions](#open-questions).

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
├── CLAUDE.md                     # Working conventions: jj, release mechanics
├── LICENSE
├── README.md
├── SPEC.md
├── .github/workflows/
│   ├── ci.yml                    # fmt, clippy, test on every push
│   └── release.yml               # Tagged builds and GitHub release
├── scripts/
│   └── check-nu-metadata.sh      # Guards the +nu- claim against Cargo.lock
└── src/
    ├── main.rs                   # Entry point: serve_plugin()
    ├── lib.rs                    # Plugin struct, registers commands
    └── to_xlsx.rs                # `to xlsx` command + inline tests
```

## Build & install

```bash
cargo build --release
plugin add target/release/nu_plugin_xlsx
plugin use xlsx
```

Both commands are needed, and they do different things. `plugin add` records the
binary's signatures in the registry file (`$nu.plugin-path`) and explicitly
*does not* bring the commands into scope. `plugin use` is the parser keyword
that loads them from the registry into scope — and it loads them for the
current session only, so it belongs in the autoload config for the commands to
survive a restart. Registering at install time is not enough.

Installing from crates.io requires a Nushell matching the release's `+nu-`
metadata; see [Nushell version compatibility](#nushell-version-compatibility).

## Design principles

1. **Idiomatic Nushell.** Follow the conventions of `to csv`, `to json`, `from xlsx`. Emit binary, accept pipeline input, use standard flags.
2. **Correct by default.** Type mapping should just work without flags. A bare `ls | to xlsx | save files.xlsx` should produce a well-formatted spreadsheet.
3. **Progressive formatting.** Zero-config output is good. Flags unlock better output. A future config-record system unlocks full `rust_xlsxwriter` power.
4. **Symmetry.** `from xlsx` and `to xlsx` should be inverses: `data | to xlsx | from xlsx` should round-trip cleanly.

## Implementation notes

- **Plugin trait**: `PluginCommand` with `PipelineData` — collects input via `into_value()`.
- **Binary output**: `Value::binary(bytes, span)`.
- **Binary passthrough**: If input is already binary, pass it through (handles `to xlsx | save foo.xlsx`).
- **Input/output types**: Two variants — `Type::table()` (sugar) and `Type::record()` (multi-sheet). Both produce `Type::Binary`. Use the helpers, never the raw enum variants: the helpers are the API's stability surface across Nushell minors.
- **Error handling**: `LabeledError` with `Span` from the source value.
- **Lints**: `clippy::pedantic`, deny `unsafe_code` and `unwrap_used`.
- **Edition**: Rust 2021 (matches `nu-plugin`).

## Open questions

- [ ] Which `rust_xlsxwriter` Table style to use as default?
- [ ] **Should `to xlsx` accept a caller-supplied schema for empty input?**
      Users ask for the header row to survive an empty dataset. Since the
      schema cannot arrive with the value ([Empty input](#empty-input)), the
      only way is a flag — `--columns [a b]`. Two sub-decisions, and Excel
      forces the second:
      - Does the flag apply only when input is empty, or always (validating
        or reordering columns when it is not)?
      - At zero rows, emit a Table with its mandatory reserved row, or plain
        header cells with no Table? A `ListObject` cannot be header-only, so
        this fork has no neutral answer. It turns on whether the user wants
        column names in the file or a Table to build on (structured
        references, pivots, data entry).

      Adding a flag widens the command surface, so this is a design call, not
      an implementation detail.
- [ ] Should releases cover more platforms? Builds currently ship
      `aarch64-apple-darwin` and `x86_64-pc-windows-msvc` only — no Linux, no
      Intel macOS.
