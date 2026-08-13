#!/usr/bin/env bash
#
# KOOMPI KESA installer.
#
#   curl -fsSL https://raw.githubusercontent.com/koompi/koompi-code-cli/main/install.sh | bash
#
# Downloads the release binary for this machine, checks it against the
# release's SHA256SUMS, and drops `kesa` in ~/.local/bin.
#
# Environment overrides: OWNER, REPO, VERSION, DEST, KESA_NO_VERIFY.

set -euo pipefail

OWNER="${OWNER:-koompi}"
REPO="${REPO:-koompi-code-cli}"
VERSION="${VERSION:-}"
DEST="${DEST:-$HOME/.local/bin}"
NO_VERIFY="${KESA_NO_VERIFY:-0}"
BIN="kesa"

if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
  C_DIM=$'\033[2m'; C_RED=$'\033[31m'; C_GREEN=$'\033[32m'; C_BOLD=$'\033[1m'; C_OFF=$'\033[0m'
else
  C_DIM=""; C_RED=""; C_GREEN=""; C_BOLD=""; C_OFF=""
fi

say()  { printf '%s\n' "$*"; }
info() { printf '%s%s%s\n' "$C_DIM" "$*" "$C_OFF"; }
ok()   { printf '%s✓%s %s\n' "$C_GREEN" "$C_OFF" "$*"; }
die()  { printf '%s✗%s %s\n' "$C_RED" "$C_OFF" "$*" >&2; exit 1; }

usage() {
  cat <<EOF
KOOMPI KESA installer

  --version <tag>   Install a specific release, e.g. v0.2.0 (default: latest)
  --dest <dir>      Install directory (default: \$HOME/.local/bin)
  --no-verify       Skip the SHA256SUMS check
  --help            Show this message

Uninstall with:
  curl -fsSL https://raw.githubusercontent.com/${OWNER}/${REPO}/main/uninstall.sh | bash
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --version) VERSION="${2:-}"; shift 2 ;;
    --dest)    DEST="${2:-}"; shift 2 ;;
    --no-verify) NO_VERIFY=1; shift ;;
    --help|-h) usage; exit 0 ;;
    *) die "unknown option: $1 (try --help)" ;;
  esac
done

need() { command -v "$1" >/dev/null 2>&1 || die "$1 is required but not installed"; }
need tar
need uname
if command -v curl >/dev/null 2>&1; then
  FETCH() { curl -fsSL --retry 3 "$1" -o "$2"; }
  FETCH_STDOUT() { curl -fsSL --retry 3 "$1"; }
elif command -v wget >/dev/null 2>&1; then
  FETCH() { wget -qO "$2" "$1"; }
  FETCH_STDOUT() { wget -qO- "$1"; }
else
  die "curl or wget is required"
fi

# Platform. The release publishes Linux builds only; everything else is told
# how to build from source rather than handed a binary that will not run.
os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
  Linux) os_tag="linux" ;;
  Darwin)
    die "no macOS build is published yet. Build from source:
  git clone https://github.com/${OWNER}/${REPO}.git && cd ${REPO} && cargo install --path ." ;;
  *) die "unsupported operating system: $os" ;;
esac
case "$arch" in
  x86_64|amd64) arch_tag="x86_64" ;;
  aarch64|arm64) arch_tag="aarch64" ;;
  *) die "unsupported architecture: $arch" ;;
esac
PLATFORM="${os_tag}-${arch_tag}"

if [ -z "$VERSION" ]; then
  info "Resolving the latest release..."
  VERSION="$(FETCH_STDOUT "https://api.github.com/repos/${OWNER}/${REPO}/releases/latest" \
    | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n1)"
  [ -n "$VERSION" ] || die "could not resolve the latest release of ${OWNER}/${REPO}"
fi

ASSET="${BIN}-${VERSION}-${PLATFORM}.tar.gz"
# KESA_DOWNLOAD_BASE points the download at a mirror. Bandwidth to GitHub from
# Cambodia is the reason this exists.
BASE="${KESA_DOWNLOAD_BASE:-https://github.com/${OWNER}/${REPO}/releases/download/${VERSION}}"

TMP="$(mktemp -d)"
cleanup() { rm -rf "$TMP"; }
trap cleanup EXIT

say "${C_BOLD}KOOMPI KESA${C_OFF} ${VERSION} (${PLATFORM})"
info "Downloading ${ASSET}..."
FETCH "${BASE}/${ASSET}" "${TMP}/${ASSET}" \
  || die "no release asset ${ASSET} at ${VERSION}"

if [ "$NO_VERIFY" != "1" ]; then
  info "Verifying checksum..."
  FETCH "${BASE}/SHA256SUMS" "${TMP}/SHA256SUMS" \
    || die "SHA256SUMS is missing from release ${VERSION}; re-run with --no-verify to skip"
  expected="$(awk -v name="$ASSET" '$2 == name || $2 == "*" name { print $1 }' "${TMP}/SHA256SUMS" | head -n1)"
  [ -n "$expected" ] || die "no checksum entry for ${ASSET}"
  if command -v sha256sum >/dev/null 2>&1; then
    actual="$(sha256sum "${TMP}/${ASSET}" | awk '{print $1}')"
  elif command -v shasum >/dev/null 2>&1; then
    actual="$(shasum -a 256 "${TMP}/${ASSET}" | awk '{print $1}')"
  else
    die "sha256sum or shasum is required to verify; re-run with --no-verify to skip"
  fi
  [ "$expected" = "$actual" ] || die "checksum mismatch for ${ASSET}
  expected $expected
  actual   $actual"
  ok "Checksum verified"
fi

tar -xzf "${TMP}/${ASSET}" -C "$TMP"
[ -f "${TMP}/${BIN}" ] || die "${ASSET} does not contain a ${BIN} binary"

mkdir -p "$DEST"
# Install through a temp name and rename, so a running kesa is not clobbered
# mid-write and an interrupted install cannot leave a truncated binary.
install -m 0755 "${TMP}/${BIN}" "${DEST}/.${BIN}.new"
mv -f "${DEST}/.${BIN}.new" "${DEST}/${BIN}"
ok "Installed ${DEST}/${BIN}"

# A v0.2.0 install left `kode` here. It keeps working, against ~/.kode, which
# is how somebody ends up running two agents with two diverging configs.
if [ -e "${DEST}/kode" ]; then
  say ""
  say "${DEST}/kode is left over from the old name. Your settings and sessions"
  say "have been copied to ~/.kesa; remove the old binary once this one looks right:"
  say "  ${C_BOLD}rm ${DEST}/kode${C_OFF}"
fi

case ":${PATH}:" in
  *":${DEST}:"*) ;;
  *)
    say ""
    say "${DEST} is not on your PATH. Add this to your shell profile:"
    say "  ${C_BOLD}export PATH=\"${DEST}:\$PATH\"${C_OFF}"
    ;;
esac

say ""
"${DEST}/${BIN}" --version || true
say ""
say "Get started:  ${C_BOLD}kesa${C_OFF}"
say "Uninstall:    ${C_DIM}curl -fsSL https://raw.githubusercontent.com/${OWNER}/${REPO}/main/uninstall.sh | bash${C_OFF}"
