#!/usr/bin/env bash
# Prove that this source tree builds against a given Nushell minor and that the
# result loads into that Nushell and answers — the promise SPEC.md makes and
# the unit tests cannot check.
#
# Usage:
#   test-nu-compat.sh <nu-version>   one version, e.g. 0.114.1
#   test-nu-compat.sh --all          every version in supported-nu.txt
#
# The tree is copied under target/nu-compat/<version>/ and the two nu-* crates
# are repinned there with `cargo add`, so Cargo.toml and Cargo.lock in the
# working copy are never touched. Builds are debug: 20 s incremental, and the
# binary is functionally the release one.
#
# Finding the shell, in order: $NU if set; the mise install of that exact
# version (aqua:nushell/nushell); else whatever `nu` is on PATH. Whichever is
# found must report the requested version, or the run stops before building.
set -euo pipefail

cd "$(dirname "$0")/.."

if [ "${1:-}" = "--all" ]; then
  status=0
  while read -r v; do
    "$0" "$v" || status=1
  done < <(grep -v '^#' supported-nu.txt | grep .)
  exit $status
fi

v="${1:?usage: $0 <nu-version> | --all}"

if [ -n "${NU:-}" ]; then
  nu="$NU"
elif command -v mise >/dev/null 2>&1 && dir=$(mise where "aqua:nushell/nushell@$v" 2>/dev/null); then
  nu=$(find "$dir" -type f -name nu -perm -u+x | head -1)
else
  nu=$(command -v nu || true)
fi
if [ -z "$nu" ]; then
  echo "error: no Nushell $v found — set NU, or: mise install aqua:nushell/nushell@$v" >&2
  exit 1
fi
actual=$("$nu" --version)
if [ "$actual" != "$v" ]; then
  echo "error: $nu is Nushell $actual, not $v" >&2
  exit 1
fi

tree="target/nu-compat/$v"
mkdir -p "$tree"
rm -rf "$tree/src"
cp -r src Cargo.toml Cargo.lock "$tree/"

echo "== Nushell $v: repin and build"
(
  cd "$tree"
  cargo add --quiet "nu-plugin@=$v"
  cargo add --quiet "nu-protocol@=$v" --features plugin
  cargo build --quiet
)

echo "== Nushell $v: load and smoke"
bin="$tree/target/debug/nu_plugin_xlsx"
reg=$(mktemp --suffix .msgpackz 2>/dev/null || mktemp)
trap 'rm -f "$reg"' EXIT
"$nu" --plugin-config "$reg" --plugins "$bin" -- scripts/smoke.nu
