#!/usr/bin/env bash
#
# KOOMPI KESA uninstaller.
#
#   curl -fsSL https://raw.githubusercontent.com/koompi/koompi-code-cli/main/uninstall.sh | bash
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
  --purge        Also delete ~/.kesa (config, sessions, credentials)
EOF
      exit 0 ;;
    *) printf 'unknown option: %s\n' "$1" >&2; exit 1 ;;
  esac
done

removed=0
for candidate in "${DEST}/${BIN}" "$(command -v "$BIN" 2>/dev/null || true)"; do
  if [ -n "$candidate" ] && [ -f "$candidate" ]; then
    rm -f "$candidate" && ok "Removed ${candidate}" && removed=1
  fi
done
[ "$removed" = 1 ] || say "No ${BIN} binary found."

if [ "$PURGE" = 1 ] && [ -d "$HOME/.kesa" ]; then
  rm -rf "$HOME/.kesa"
  ok "Removed ~/.kesa"
elif [ -d "$HOME/.kesa" ]; then
  say "${C_DIM}Config kept at ~/.kesa. Pass --purge to delete it.${C_OFF}"
fi
