#!/usr/bin/env bash
#
# KOOMPI KESA uninstaller.
#
#   curl -fsSL https://raw.githubusercontent.com/koompi/koompi-kesa/main/uninstall.sh | bash
#
# Removes the `kesa` binary. Config, sessions and credentials under ~/.kesa are
# left alone unless you pass --purge.

set -euo pipefail

DEST="${DEST:-$HOME/.local/bin}"
PURGE=0
BIN="kesa"

if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
  C_DIM=$'\033[2m'; C_GREEN=$'\033[32m'; C_OFF=$'\033[0m'
else
  C_DIM=""; C_GREEN=""; C_OFF=""
fi
say()  { printf '%s\n' "$*"; }
ok()   { printf '%s✓%s %s\n' "$C_GREEN" "$C_OFF" "$*"; }

while [ $# -gt 0 ]; do
  case "$1" in
    --dest)  DEST="${2:-}"; shift 2 ;;
    --purge) PURGE=1; shift ;;
    --help|-h)
      cat <<EOF
KOOMPI KESA uninstaller

  --dest <dir>   Where kesa was installed (default: \$HOME/.local/bin)
  --purge        Also delete ~/.kesa and ~/.kode (config, sessions, credentials)
EOF
      exit 0 ;;
    *) printf 'unknown option: %s\n' "$1" >&2; exit 1 ;;
  esac
done

# kode is the pre-rename binary; leaving it behind leaves a working agent on PATH
removed=0
for candidate in "${DEST}/${BIN}" "${DEST}/kode"; do
  [ -f "$candidate" ] || continue
  if rm -f "$candidate"; then
    ok "Removed ${candidate}"
    removed=1
  else
    say "Could not remove ${candidate}. Remove it by hand, or re-run with a writable --dest." >&2
    exit 1
  fi
done
[ "$removed" = 1 ] || say "No ${BIN} binary found in ${DEST}."

# anything this installer did not place is the user's or a package manager's
elsewhere="$(command -v "$BIN" 2>/dev/null || true)"
if [ -n "$elsewhere" ] && [ "$elsewhere" != "${DEST}/${BIN}" ]; then
  say ""
  say "Another ${BIN} is still on your PATH at ${elsewhere}."
  say "This uninstaller only removes what it installed. Remove that one with:"
  say "  rm ${elsewhere}"
fi

for dir in "$HOME/.kesa" "$HOME/.kode"; do
  [ -d "$dir" ] || continue
  if [ "$PURGE" = 1 ]; then
    rm -rf "$dir"
    ok "Removed ${dir}"
  else
    say "${C_DIM}Config kept at ${dir}. Pass --purge to delete it.${C_OFF}"
  fi
done
