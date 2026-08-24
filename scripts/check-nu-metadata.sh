#!/usr/bin/env bash
# Verify that the version's +nu-<version> build metadata matches the
# nu-plugin version Cargo.lock actually resolves to.
#
# Why this needs checking: the plugin protocol version is baked in at compile
# time (nu-plugin-protocol sets it from CARGO_PKG_VERSION) and Nushell rejects
# any plugin outside a caret match, so ^0.114.1 excludes 0.115.0. Every Nushell
# minor is a hard break. The +nu- metadata is how a release states which
# Nushell it loads into — an unchecked claim ships a binary that cannot load.
#
# Usage:
#   check-nu-metadata.sh            warn if metadata is absent, fail if it is wrong
#   check-nu-metadata.sh --strict   also fail if metadata is absent (release gate)
set -euo pipefail

strict=false
[ "${1:-}" = "--strict" ] && strict=true

cd "$(dirname "$0")/.."

manifest=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)

locked=$(awk '/^name = "nu-plugin"$/{f=1;next} f && /^version = /{print $3; exit}' Cargo.lock | tr -d '"')
if [ -z "$locked" ]; then
  echo "error: could not find nu-plugin in Cargo.lock" >&2
  exit 1
fi

case "$manifest" in
  *+nu-*)
    declared="${manifest#*+nu-}"
    if [ "$declared" != "$locked" ]; then
      echo "error: version claims nu $declared but Cargo.lock resolves nu-plugin $locked" >&2
      exit 1
    fi
    echo "Targets Nushell $declared"
    ;;
  *)
    # Absent metadata is a release blocker but a normal mid-development state:
    # an infra commit lands before the bump commit that restates the target.
    msg="version '$manifest' carries no +nu-<version> build metadata (expected e.g. 0.2.1+nu-$locked)"
    if $strict; then
      echo "error: $msg" >&2
      exit 1
    fi
    echo "warning: $msg"
    echo "Cargo.lock resolves nu-plugin $locked"
    ;;
esac
