# nu_plugin_xlsx

**Any Nushell table, straight into a spreadsheet that opens looking finished.**

```nushell
ls | save files.xlsx
```

That is the whole API. No flags, no schema, no template. And what lands in Excel is not a
CSV with a new suffix: it is a real Excel Table — filter buttons on the header, banded rows,
columns sized to their content, dates Excel knows are dates, numbers it can sum. Your
colleague opens it and starts sorting.

Nushell already reads spreadsheets with `from xlsx`. This is the other half.

## What it does

- **One line per workbook.** `save` sees the `.xlsx` extension and calls `to xlsx` for you.
- **One sheet per key.** Pipe a record of tables; each key becomes a named sheet, in order.
- **Types survive.** Integers, floats, booleans, real dates. File sizes land as bytes,
  durations as seconds, `null` as an empty cell. Lists and records are written as text.
- **A Table, not a range.** Auto-filter, banded rows, autofit — pivot-ready on arrival.
  `--raw` gives plain cells if that is what you want.
- **Anything Nushell can tabulate.** `ls`, `ps`, `http get`, `open data.json`, a database
  query. If it is a table, it is a sheet.

```nushell
# Two sheets, one line
{ Files: (ls), Processes: (ps | first 20) } | save snapshot.xlsx

# A report from an API
let orders = http get https://api.example.com/orders
{ Orders: $orders, Customers: ($orders | select customer email | uniq) } | save report.xlsx

# Bytes go wherever bytes go
ls | to xlsx | http post https://example.com/upload
```

## Install

```nushell
http get https://raw.githubusercontent.com/ChristianLemer/nu_plugin_xlsx/HEAD/install.nu | save -f install.nu
nu install.nu --register
```

The installer reads the Nushell running it, downloads the matching build, verifies its
checksum and registers it. No Rust toolchain. Then add `plugin use xlsx` to your config so
the command survives a restart — `plugin add` registers, it does not load.

Builds exist for the current Nushell minor and the two before it — the exact list is
[supported-nu.txt](supported-nu.txt). Check yours with `version | get version`.

> **Re-run the installer after every Nushell upgrade.** A plugin binary loads into exactly
> one Nushell minor, and the failure when it does not is mute: `plugin add` reports
> `Failed to send plugin call` or `nu::shell::io::broken_pipe`, never a version. The
> installer keeps a copy of itself beside the binary, so re-running is local:
>
> ```nushell
> nu ($nu.data-dir | path join plugins install.nu) --register
> ```

<details>
<summary><b>Install by hand from a release download</b></summary>

Assets on [Releases](https://github.com/ChristianLemer/nu_plugin_xlsx/releases) are named
`nu_plugin_xlsx-nu<nu-version>-<target>.tar.gz` (`.zip` on Windows), one per platform:

| Target | For |
| --- | --- |
| `x86_64-unknown-linux-musl` | any Linux on Intel or AMD — statically linked, no glibc requirement |
| `aarch64-unknown-linux-musl` | any Linux on ARM — a Raspberry Pi, an ARM server or cloud instance (from the next release) |
| `aarch64-apple-darwin` | Apple Silicon |
| `x86_64-apple-darwin` | Intel Mac |
| `x86_64-pc-windows-msvc` | Windows |

Pick the one whose `nu<nu-version>` matches yours. Extract it — the binary inside is already
named `nu_plugin_xlsx`, which matters because Nushell refuses to register a file whose name
doesn't start with `nu_plugin_`:

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

</details>

<details>
<summary><b>Install from crates.io</b></summary>

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

</details>

<details>
<summary><b>Uninstall</b></summary>

```nushell
plugin rm xlsx
```

Then delete the binary the installer put in `$nu.data-dir | path join plugins`, and drop
`plugin use xlsx` from your config.

</details>

## Good to know

**Sheet names are Excel's rules, not ours.** At most 31 characters, not empty, none of
`[ ] : * ? / \`, unique regardless of case. A key that breaks one fails the conversion with
`Failed to set sheet name`; rename the key first:

```nushell
$report | rename --column { "Q1/Q2 2024": "Q1-Q2 2024" } | save report.xlsx
```

**Reading it back needs no plugin.** Nushell's own `from xlsx` does it, first row as column
names. Excel stores every number as a float, so `30` comes back as `30.00` unless you ask
(the flag exists from Nushell 0.114):

```nushell
open --raw report.xlsx | from xlsx --prefer-integers | get Orders
```

**An empty table is still a workbook**, with an empty `Sheet1`. Its column names cannot
survive: an empty table carries no schema in Nushell, so there is nothing to write.

**A row missing a column** that other rows have gets an empty cell there.

## When it goes wrong

| You see | It means | Do |
| --- | --- | --- |
| `Failed to send plugin call`, `broken_pipe` | binary built for another Nushell minor | re-run the installer |
| `Failed to set sheet name` | a key breaks a sheet-name rule | rename the key, see above |
| `Command does not support binary input` | xlsx bytes reached `to xlsx` twice | drop the extra `to xlsx` |
| commands gone after restart | registered, not loaded | `plugin use xlsx` in your config |

## Contributing

The gates CI runs, plus the check that one source tree still serves every supported
Nushell minor:

```bash
cargo fmt -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
./scripts/test-nu-compat.sh --all
```

[CLAUDE.md](CLAUDE.md) has the setup for that last one. [SPEC.md](SPEC.md) holds the design
decisions and their reasons; read it before widening the command's surface.

## About

This is my first open-source project. I've been a software engineer in enterprise environments for years but hadn't published anything publicly until now.

This project is developed with [Claude Code](https://claude.ai/claude-code) as a pair programming partner. Design decisions, architecture, and code review are human-driven — AI assists with implementation, research, and iteration.

## License

MIT
