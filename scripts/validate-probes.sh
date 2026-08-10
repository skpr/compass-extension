#!/bin/sh
#
# Validates the USDT probe contract against probes.spec.
#
# probes.spec is the source of truth. Three consumers must agree with it:
#   1. the built .so's ELF notes - the probes that actually exist
#   2. bpftrace/compass.bt       - the reference tracer
#   3. README.md                 - the documentation
#
# Running this also forces the cdylib to be linked, which nothing else in CI
# does - `cargo clippy` only emits metadata. A missing or renamed PHP symbol for
# a given PHP version cannot fail CI without this.
#
# Deliberately register-agnostic; see the note in probes.spec.
#
# Usage: scripts/validate-probes.sh [path to .so]

set -eu

ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
SO=${1:-"${CARGO_TARGET_DIR:-$ROOT/target}/release/libcompass_extension.so"}
SPEC="$ROOT/probes.spec"
BT="$ROOT/bpftrace/compass.bt"
DOC="$ROOT/README.md"

[ -f "$SO" ] || { echo "FAIL: no such library: $SO (run 'mise run build' first)" >&2; exit 1; }
[ -f "$SPEC" ] || { echo "FAIL: no such spec: $SPEC" >&2; exit 1; }

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

# A counter rather than a flag, so each section can tell whether *it* failed by
# snapshotting the count. A single global flag would leave later sections
# reporting "ok" once any earlier section had tripped it.
failures=0
report() { echo "FAIL: $1"; failures=$((failures + 1)); }

# Expected: "<name> <argc>", comments and blank lines stripped.
sed -e 's/#.*//' -e '/^[[:space:]]*$/d' "$SPEC" \
  | awk '{ print $1, $2 }' | sort > "$WORK/expected"

# Actual, from the ELF notes. "Arguments:" with no operands means zero arguments.
readelf -n "$SO" 2>/dev/null | awk '
  /^[[:space:]]*Provider:/  { provider = $2 }
  /^[[:space:]]*Name:/      { name = $2 }
  /^[[:space:]]*Arguments:/ {
    if (provider == "compass" && name != "") {
      argc = NF - 1
      if (argc < 0) argc = 0
      print name, argc
    }
  }
' | sort > "$WORK/actual"

# ---- 1. the binary matches the spec ------------------------------------------
if diff -u "$WORK/expected" "$WORK/actual" > "$WORK/diff" 2>&1; then
  echo "ok: $(wc -l < "$WORK/actual" | tr -d ' ') probes in $(basename "$SO") match probes.spec"
else
  report "the .so does not match probes.spec (-expected +actual)"
  sed 's/^/    /' "$WORK/diff"
fi

awk '{ print $1 }' "$WORK/expected" > "$WORK/names"

# ---- 2. the reference tracer uses probes that exist, within their arity ------
if [ -f "$BT" ]; then
  before=$failures

  awk '
    match($0, /:compass:[A-Za-z0-9_]+/) {
      current = substr($0, RSTART + 9, RLENGTH - 9)
      used[current] = 1
      next
    }
    {
      if (current == "") next
      line = $0
      while (match(line, /arg[0-9]+/)) {
        n = substr(line, RSTART + 3, RLENGTH - 3) + 1
        if (n > highest[current]) highest[current] = n
        line = substr(line, RSTART + RLENGTH)
      }
      if ($0 ~ /^[[:space:]]*}/) current = ""
    }
    END { for (p in used) print p, highest[p] + 0 }
  ' "$BT" | sort > "$WORK/bt"

  while read -r probe needed; do
    argc=$(awk -v p="$probe" '$1 == p { print $2 }' "$WORK/expected")
    if [ -z "$argc" ]; then
      report "$(basename "$BT") traces '$probe', which is not in probes.spec"
    elif [ "$needed" -gt "$argc" ]; then
      report "$(basename "$BT") reads arg$((needed - 1)) of '$probe', which takes $argc argument(s)"
    fi
  done < "$WORK/bt"

  # Every probe should be demonstrated by the reference tracer.
  while read -r probe; do
    awk -v p="$probe" '$1 == p { found = 1 } END { exit !found }' "$WORK/bt" \
      || report "$(basename "$BT") does not trace '$probe'"
  done < "$WORK/names"

  [ "$failures" -eq "$before" ] && echo "ok: $(basename "$BT") agrees with probes.spec"
fi

# ---- 3. every probe is documented -------------------------------------------
# Catches exactly the drift that shipped in c7e6a41, where the code was renamed
# to cli_request_init/cli_function but the README kept the old names.
if [ -f "$DOC" ]; then
  before=$failures
  while read -r probe; do
    grep -q -- "\`$probe\`" "$DOC" \
      || report "$(basename "$DOC") does not document '$probe'"
  done < "$WORK/names"
  [ "$failures" -eq "$before" ] && echo "ok: $(basename "$DOC") documents every probe"
fi

if [ "$failures" -eq 0 ]; then
  echo "probe contract OK"
  exit 0
fi

echo "probe contract FAILED: $failures problem(s)"
exit 1
