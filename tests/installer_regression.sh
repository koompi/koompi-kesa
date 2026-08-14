#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INSTALLER="${ROOT}/install.sh"
WORK_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/kesa-installer-regression-XXXXXX")"

PASS_COUNT=0
FAIL_COUNT=0
KEEP_WORK="${KESA_INSTALLER_TEST_KEEP:-0}"

cleanup() {
  [ "$KEEP_WORK" = "1" ] || rm -rf "$WORK_ROOT"
}
trap cleanup EXIT

usage() {
  cat <<'USAGE'
Usage: tests/installer_regression.sh

Regression checks for install.sh, run against a release served from a local
directory over file:// so no case touches the network. Covers:
  - option parsing and --help
  - platform to release-asset mapping
  - the SHA256SUMS verification branches
  - what lands in --dest

Set KESA_INSTALLER_TEST_KEEP=1 to keep the scratch directory.
USAGE
}

sha256_of() {
  local file="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file" | awk '{print $1}'
  else
    echo "missing sha256 tool (sha256sum or shasum)" >&2
    return 1
  fi
}

case_dir() {
  local dir="${WORK_ROOT}/$1"
  mkdir -p "${dir}/home" "${dir}/dest" "${dir}/release" "${dir}/bin" "${dir}/stage"
  printf '%s\n' "$dir"
}

# A release as release.yml publishes one: per-platform tarballs plus a
# SHA256SUMS in `sha256sum` format.
write_release() {
  local dir="$1" version="$2" platform sum
  printf '#!/usr/bin/env bash\necho "kesa %s (fixture)"\n' "$version" > "${dir}/stage/kesa"
  chmod +x "${dir}/stage/kesa"
  : > "${dir}/release/SHA256SUMS"
  for platform in linux-x86_64 linux-aarch64; do
    tar -czf "${dir}/release/kesa-${version}-${platform}.tar.gz" -C "${dir}/stage" kesa
    sum="$(sha256_of "${dir}/release/kesa-${version}-${platform}.tar.gz")"
    printf '%s  kesa-%s-%s.tar.gz\n' "$sum" "$version" "$platform" >> "${dir}/release/SHA256SUMS"
  done
}

write_uname_stub() {
  local dir="$1" os="$2" arch="$3"
  cat > "${dir}/bin/uname" <<STUB
#!/usr/bin/env bash
case "\${1:-}" in
  -s) echo "${os}" ;;
  -m) echo "${arch}" ;;
  *)  echo "${os}" ;;
esac
STUB
  chmod +x "${dir}/bin/uname"
}

run_installer() {
  local dir="$1"
  shift
  local rc=0
  (
    HOME="${dir}/home" \
    NO_COLOR=1 \
    KESA_DOWNLOAD_BASE="file://${dir}/release" \
    PATH="${dir}/bin:${PATH}" \
    bash "${INSTALLER}" "$@"
  ) >"${dir}/output.log" 2>&1 || rc=$?
  printf '%s\n' "$rc" > "${dir}/exit_code"
}

dump() {
  echo "--- output (${1}) ---" >&2
  cat "${1}/output.log" >&2
}

assert_exit_code() {
  local dir="$1" expected="$2" actual
  actual="$(cat "${dir}/exit_code")"
  if [ "$actual" != "$expected" ]; then
    echo "expected exit ${expected}, got ${actual}" >&2
    dump "$dir"
    return 1
  fi
}

assert_exit_nonzero() {
  local dir="$1" actual
  actual="$(cat "${dir}/exit_code")"
  if [ "$actual" = "0" ]; then
    echo "expected a non-zero exit, got 0" >&2
    dump "$dir"
    return 1
  fi
}

assert_output_contains() {
  local dir="$1" needle="$2"
  if ! grep -Fq -- "$needle" "${dir}/output.log"; then
    echo "missing output text: ${needle}" >&2
    dump "$dir"
    return 1
  fi
}

refute_output_contains() {
  local dir="$1" needle="$2"
  if grep -Fq -- "$needle" "${dir}/output.log"; then
    echo "unexpected output text: ${needle}" >&2
    dump "$dir"
    return 1
  fi
}

assert_installed() {
  local dir="$1"
  [ -x "${dir}/dest/kesa" ] || {
    echo "expected an executable at ${dir}/dest/kesa" >&2
    dump "$dir"
    return 1
  }
}

refute_installed() {
  local dir="$1"
  [ ! -e "${dir}/dest/kesa" ] || {
    echo "a failed install must not leave ${dir}/dest/kesa behind" >&2
    dump "$dir"
    return 1
  }
}

run_test() {
  local name="$1" status
  set +e
  (
    set -e
    "$name"
  )
  status=$?
  set -e
  if [ "$status" -eq 0 ]; then
    PASS_COUNT=$((PASS_COUNT + 1))
    echo "[PASS] ${name}"
  else
    FAIL_COUNT=$((FAIL_COUNT + 1))
    echo "[FAIL] ${name}"
  fi
}

test_help_lists_the_shipped_flags() {
  local dir
  dir="$(case_dir help-flags)"
  run_installer "$dir" --help
  assert_exit_code "$dir" 0
  assert_output_contains "$dir" "--version <tag>"
  assert_output_contains "$dir" "--dest <dir>"
  assert_output_contains "$dir" "--no-verify"
  assert_output_contains "$dir" "--help"
}

test_unknown_option_fails() {
  local dir
  dir="$(case_dir unknown-option)"
  run_installer "$dir" --totally-unknown-flag
  assert_exit_code "$dir" 1
  assert_output_contains "$dir" "unknown option: --totally-unknown-flag"
}

test_missing_option_value_installs_nothing() {
  local dir
  dir="$(case_dir missing-option-value)"
  write_release "$dir" v9.9.9
  run_installer "$dir" --version
  assert_exit_nonzero "$dir"
  refute_installed "$dir"
}

test_linux_x86_64_uses_the_published_asset_name() {
  local dir
  dir="$(case_dir linux-x86-64)"
  write_uname_stub "$dir" Linux x86_64
  write_release "$dir" v9.9.9
  run_installer "$dir" --version v9.9.9 --dest "${dir}/dest"
  assert_exit_code "$dir" 0
  assert_output_contains "$dir" "kesa-v9.9.9-linux-x86_64.tar.gz"
  assert_installed "$dir"
}

test_linux_aarch64_uses_the_published_asset_name() {
  local dir
  dir="$(case_dir linux-aarch64)"
  write_uname_stub "$dir" Linux aarch64
  write_release "$dir" v9.9.9
  run_installer "$dir" --version v9.9.9 --dest "${dir}/dest"
  assert_exit_code "$dir" 0
  assert_output_contains "$dir" "kesa-v9.9.9-linux-aarch64.tar.gz"
  assert_installed "$dir"
}

test_macos_is_refused_with_build_from_source_guidance() {
  local dir
  dir="$(case_dir macos-refused)"
  write_uname_stub "$dir" Darwin arm64
  write_release "$dir" v9.9.9
  run_installer "$dir" --version v9.9.9 --dest "${dir}/dest"
  assert_exit_code "$dir" 1
  assert_output_contains "$dir" "no macOS build is published yet"
  assert_output_contains "$dir" "cargo install --path ."
  refute_installed "$dir"
}

test_unsupported_architecture_is_refused() {
  local dir
  dir="$(case_dir unsupported-arch)"
  write_uname_stub "$dir" Linux riscv64
  write_release "$dir" v9.9.9
  run_installer "$dir" --version v9.9.9 --dest "${dir}/dest"
  assert_exit_code "$dir" 1
  assert_output_contains "$dir" "unsupported architecture: riscv64"
  refute_installed "$dir"
}

test_missing_release_asset_fails() {
  local dir
  dir="$(case_dir missing-asset)"
  write_release "$dir" v9.9.9
  run_installer "$dir" --version v0.0.0 --dest "${dir}/dest"
  assert_exit_code "$dir" 1
  assert_output_contains "$dir" "no release asset"
  refute_installed "$dir"
}

test_checksum_verified_install_succeeds() {
  local dir
  dir="$(case_dir checksum-verified)"
  write_release "$dir" v9.9.9
  run_installer "$dir" --version v9.9.9 --dest "${dir}/dest"
  assert_exit_code "$dir" 0
  assert_output_contains "$dir" "Checksum verified"
  assert_output_contains "$dir" "kesa v9.9.9 (fixture)"
  assert_installed "$dir"
  [ "$(stat -c '%a' "${dir}/dest/kesa" 2>/dev/null || stat -f '%Lp' "${dir}/dest/kesa")" = "755" ] || {
    echo "installed binary should be mode 0755" >&2
    return 1
  }
  # The install goes through .kesa.new and renames; the temp name must not survive.
  [ ! -e "${dir}/dest/.kesa.new" ] || {
    echo "install left its temporary name behind" >&2
    return 1
  }
}

test_checksum_mismatch_aborts_without_installing() {
  local dir
  dir="$(case_dir checksum-mismatch)"
  write_release "$dir" v9.9.9
  printf '%s  kesa-v9.9.9-linux-x86_64.tar.gz\n%s  kesa-v9.9.9-linux-aarch64.tar.gz\n' \
    "$(printf '0%.0s' {1..64})" "$(printf '0%.0s' {1..64})" > "${dir}/release/SHA256SUMS"
  run_installer "$dir" --version v9.9.9 --dest "${dir}/dest"
  assert_exit_code "$dir" 1
  assert_output_contains "$dir" "checksum mismatch"
  refute_installed "$dir"
}

test_missing_checksum_entry_aborts_without_installing() {
  local dir
  dir="$(case_dir checksum-no-entry)"
  write_release "$dir" v9.9.9
  printf '%s  some-other-artifact.tar.gz\n' "$(printf '1%.0s' {1..64})" > "${dir}/release/SHA256SUMS"
  run_installer "$dir" --version v9.9.9 --dest "${dir}/dest"
  assert_exit_code "$dir" 1
  assert_output_contains "$dir" "no checksum entry for"
  refute_installed "$dir"
}

test_missing_sha256sums_aborts_and_names_the_escape_hatch() {
  local dir
  dir="$(case_dir checksum-manifest-missing)"
  write_release "$dir" v9.9.9
  rm -f "${dir}/release/SHA256SUMS"
  run_installer "$dir" --version v9.9.9 --dest "${dir}/dest"
  assert_exit_code "$dir" 1
  assert_output_contains "$dir" "SHA256SUMS is missing from release v9.9.9"
  assert_output_contains "$dir" "--no-verify"
  refute_installed "$dir"
}

test_no_verify_skips_the_checksum_check() {
  local dir
  dir="$(case_dir no-verify)"
  write_release "$dir" v9.9.9
  rm -f "${dir}/release/SHA256SUMS"
  run_installer "$dir" --version v9.9.9 --dest "${dir}/dest" --no-verify
  assert_exit_code "$dir" 0
  refute_output_contains "$dir" "Checksum verified"
  assert_installed "$dir"
}

test_leftover_kode_binary_is_reported() {
  local dir
  dir="$(case_dir leftover-kode)"
  write_release "$dir" v9.9.9
  printf '#!/usr/bin/env bash\necho kode\n' > "${dir}/dest/kode"
  chmod +x "${dir}/dest/kode"
  run_installer "$dir" --version v9.9.9 --dest "${dir}/dest"
  assert_exit_code "$dir" 0
  assert_output_contains "$dir" "is left over from the old name"
  assert_output_contains "$dir" "rm ${dir}/dest/kode"
  [ -e "${dir}/dest/kode" ] || {
    echo "the installer must not delete the old binary itself" >&2
    return 1
  }
}

main() {
  if [ "${1:-}" = "-h" ] || [ "${1:-}" = "--help" ]; then
    usage
    exit 0
  fi

  [ -f "$INSTALLER" ] || {
    echo "installer not found: ${INSTALLER}" >&2
    exit 1
  }

  run_test test_help_lists_the_shipped_flags
  run_test test_unknown_option_fails
  run_test test_missing_option_value_installs_nothing
  run_test test_linux_x86_64_uses_the_published_asset_name
  run_test test_linux_aarch64_uses_the_published_asset_name
  run_test test_macos_is_refused_with_build_from_source_guidance
  run_test test_unsupported_architecture_is_refused
  run_test test_missing_release_asset_fails
  run_test test_checksum_verified_install_succeeds
  run_test test_checksum_mismatch_aborts_without_installing
  run_test test_missing_checksum_entry_aborts_without_installing
  run_test test_missing_sha256sums_aborts_and_names_the_escape_hatch
  run_test test_no_verify_skips_the_checksum_check
  run_test test_leftover_kode_binary_is_reported

  echo ""
  echo "work dir: ${WORK_ROOT}"
  echo "passed:   ${PASS_COUNT}"
  echo "failed:   ${FAIL_COUNT}"

  if [ "${FAIL_COUNT}" -gt 0 ]; then
    exit 1
  fi
}

main "$@"
