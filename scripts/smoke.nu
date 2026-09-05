# Exercise `to xlsx` inside a real Nushell, through the plugin protocol.
#
# The unit tests call the Rust function directly; nothing there proves that a
# binary loads into a given Nushell and answers on the wire. This script does.
# Run it with the plugin loaded for the session only, so a registered build
# cannot answer in place of the one under test:
#
#   nu --plugin-config <scratch> --plugins <path/to/nu_plugin_xlsx> -- scripts/smoke.nu
#
# Every case round-trips through Nushell's own `from xlsx`. Excel has no
# integer type, so numbers come back as floats: the expectations say 1.0.
use std/assert

# Single table: sugar for {Sheet1: $table}.
assert equal ([[a b]; [1 2]] | to xlsx | from xlsx) {Sheet1: [[a b]; [1.0 2.0]]}

# Record of tables: one sheet per key, in key order.
let multi = ({one: [[x]; [1]], two: [[y]; ["s"]]} | to xlsx | from xlsx)
assert equal ($multi | columns) [one two]
assert equal $multi.two.0.y "s"

# Empty table: a workbook with an empty Sheet1, not an error.
assert equal ([] | to xlsx | from xlsx) {Sheet1: []}

# Type mapping: string, float, bool survive; a date reads back as a datetime.
let row = ([[f s b d]; [1.5 "x" true 2024-01-02]] | to xlsx | from xlsx | get Sheet1.0)
assert equal $row.f 1.5
assert equal $row.s "x"
assert equal $row.b true
assert equal ($row.d | describe) "datetime"
assert equal ($row.d | format date "%Y-%m-%d") "2024-01-02"

# Wrong input is an error, not a workbook. The parser rejects a literal string
# before runtime, so it goes through an `any` to reach the plugin.
let bad: any = "not a table"
assert (try { $bad | to xlsx; false } catch { true }) "a string must be rejected"

print $"smoke ok · Nushell ((version).version)"
