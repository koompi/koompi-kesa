#!/usr/bin/env bash
#
# KOOMPI KESA uninstaller.
#
#   curl -fsSL https://raw.githubusercontent.com/koompi/koompi-kesa/main/uninstall.sh | bash
#
# Removes the `kesa` binary. Config, sessions and credentials under ~/.kesa are
# left alone unless you pass --purge, which asks before it deletes them.

set -euo pipefail

DEST="${DEST:-$HOME/.local/bin}"
PURGE=0
ASSUME_YES=0
BIN="kesa"

if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
  C_DIM=$'\033[2m'; C_GREEN=$'\033[32m'; C_OFF=$'\033[0m'
else
  C_DIM=""; C_GREEN=""; C_OFF=""
fi
say()  { printf '%s\n' "$*"; }
ok()   { printf '%s✓%s %s\n' "$C_GREEN" "$C_OFF" "$*"; }
die()  { printf '%s\n' "$*" >&2; exit 1; }

# shift 2 with no value exits 1 silently under set -e
need_value() { [ $# -ge 2 ] && [ -n "${2:-}" ] || die "$1 needs a value (try --help)"; }

while [ $# -gt 0 ]; do
  case "$1" in
    --dest)  need_value "$@"; DEST="$2"; shift 2 ;;
    --purge) PURGE=1; shift ;;
    --yes|-y) ASSUME_YES=1; shift ;;
    --help|-h)
      cat <<EOF
KOOMPI KESA uninstaller

  --dest <dir>   Where kesa was installed (default: \$HOME/.local/bin)
  --purge        Also delete ~/.kesa and ~/.kode (config, sessions, credentials)
  --yes, -y      Do not ask before deleting them; only meaningful with --purge
EOF
      exit 0 ;;
    *) die "unknown option: $1 (try --help)" ;;
  esac
done

# CODING_AGENT_DIR moves state out of ~/.kesa; purging the default would delete nothing
if [ "$PURGE" = 1 ] && [ -n "${CODING_AGENT_DIR:-}${KESA_CODING_AGENT_DIR:-}" ]; then
  say "CODING_AGENT_DIR is set, so this agent's state is not under ~/.kesa."
  say "Delete ${CODING_AGENT_DIR:-${KESA_CODING_AGENT_DIR:-}} by hand if you want it gone."
  say ""
fi

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

# empty HOME would make the purge below rm -rf /.kesa
[ -n "${HOME:-}" ] || die "HOME is not set; refusing to guess where the config lives."

purge_dirs=""
for dir in "$HOME/.kesa" "$HOME/.kode"; do
  [ -d "$dir" ] || continue
  if [ "$PURGE" = 1 ]; then
    purge_dirs="${purge_dirs}${dir}
"
  else
    say "${C_DIM}Config kept at ${dir}. Pass --purge to delete it.${C_OFF}"
  fi
done

if [ -n "$purge_dirs" ]; then
  say ""
  say "About to delete, including saved credentials and every session:"
  printf '%s' "$purge_dirs" | while IFS= read -r dir; do say "  ${dir}"; done
  if [ "$ASSUME_YES" != 1 ]; then
    # stdin is the script under curl | bash; /dev/tty can exist and still not open
    if { exec 3<>/dev/tty; } 2>/dev/null; then
      printf 'Delete them? [y/N] ' >&3
      read -r reply <&3 || reply=""
      exec 3<&-
    else
      die "Not running on a terminal, so there is nobody to ask. Re-run with --yes to confirm."
    fi
    case "$reply" in
      y|Y|yes|YES) ;;
      *) say "Left in place."; exit 0 ;;
    esac
  fi
  printf '%s' "$purge_dirs" | while IFS= read -r dir; do
    rm -rf "$dir"
    ok "Removed ${dir}"
  done
fi
