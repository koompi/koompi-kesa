#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd -P)"

SKILL_DIR="${ROOT_DIR}/.claude/skills/pi-agent-rust"
SKILL_FILE="${SKILL_DIR}/SKILL.md"
COMMANDS_FILE="${SKILL_DIR}/references/COMMANDS.md"
DEBUG_FILE="${SKILL_DIR}/references/DEBUGGING-PLAYBOOKS.md"
INSTALLER_FILE="${ROOT_DIR}/install.sh"

ERROR_COUNT=0

ok() {
  echo "✓ $*" >&2
}

warn() {
  echo "⚠ $*" >&2
}

fail() {
  echo "✗ $*" >&2
  ERROR_COUNT=$((ERROR_COUNT + 1))
}

require_file() {
  local file="$1"
  if [ ! -f "$file" ]; then
    fail "Missing file: $file"
    return 1
  fi
  return 0
}

check_frontmatter() {
  if ! require_file "$SKILL_FILE"; then
    return 0
  fi

  local first_line
  first_line="$(head -n 1 "$SKILL_FILE" || true)"
  if [ "$first_line" != "---" ]; then
    fail "SKILL.md must start with frontmatter delimiter '---'"
    return 0
  fi

  local close_line
  close_line="$(awk 'NR > 1 && $0 == "---" { print NR; exit }' "$SKILL_FILE")"
  if [ -z "$close_line" ]; then
    fail "SKILL.md frontmatter closing delimiter not found"
    return 0
  fi

  local frontmatter
  frontmatter="$(sed -n "1,${close_line}p" "$SKILL_FILE")"
  if ! grep -Eq '^name:[[:space:]]+pi-agent-rust$' <<< "$frontmatter"; then
    fail "SKILL.md frontmatter must include: name: pi-agent-rust"
  fi
  if ! grep -Eq '^description:' <<< "$frontmatter"; then
    fail "SKILL.md frontmatter missing description field"
  fi
  if ! grep -Fq "Use when" <<< "$frontmatter"; then
    fail "SKILL.md description should include 'Use when' trigger text"
  fi
}

check_marker_and_links() {
  if ! require_file "$SKILL_FILE"; then
    return 0
  fi
  require_file "$COMMANDS_FILE" || true
  require_file "$DEBUG_FILE" || true

  if ! grep -Fq "koompi_code_cli installer managed skill" "$SKILL_FILE"; then
    fail "SKILL.md missing installer-managed marker comment"
  fi

  mapfile -t refs < <(grep -oE 'references/[A-Za-z0-9._-]+\.md' "$SKILL_FILE" | sort -u || true)
  if [ "${#refs[@]}" -eq 0 ]; then
    fail "SKILL.md should include at least one references/*.md link"
  fi
  local ref=""
  for ref in "${refs[@]}"; do
    if [ ! -f "${SKILL_DIR}/${ref}" ]; then
      fail "Missing referenced file: ${SKILL_DIR}/${ref}"
    fi
  done

  if ! grep -Fq "DEBUGGING-PLAYBOOKS.md" "$COMMANDS_FILE"; then
    fail "COMMANDS.md should link to DEBUGGING-PLAYBOOKS.md"
  fi
}

check_key_paths_exist() {
  local required_paths=(
    "src/main.rs"
    "src/agent.rs"
    "src/provider.rs"
    "src/tools.rs"
    "src/session.rs"
    "src/session_index.rs"
    "src/extensions.rs"
    "src/extensions_js.rs"
    "src/interactive.rs"
    "src/rpc.rs"
    "install.sh"
    "uninstall.sh"
    "tests/installer_regression.sh"
  )
  local rel=""
  for rel in "${required_paths[@]}"; do
    if [ ! -e "${ROOT_DIR}/${rel}" ]; then
      fail "Expected path referenced by skill is missing: ${rel}"
    fi
  done
}

extract_inline_skill() {
  awk '
    /^pi_agent_skill_inline_content\(\) \{/ {in_fn=1; next}
    in_fn && /^  cat <<'\''SKILL'\''$/ {capture=1; next}
    capture && /^SKILL$/ {exit}
    capture {print}
  ' "$INSTALLER_FILE"
}

check_inline_sync() {
  if ! require_file "$INSTALLER_FILE"; then
    return 0
  fi
  if ! require_file "$SKILL_FILE"; then
    return 0
  fi

  local expected_file inline_file diff_file
  expected_file="$(mktemp)"
  inline_file="$(mktemp)"
  diff_file="$(mktemp)"

  sed 's/[[:space:]]*$//' "$SKILL_FILE" > "$expected_file"
  extract_inline_skill | sed 's/[[:space:]]*$//' > "$inline_file"

  if [ ! -s "$inline_file" ]; then
    fail "Failed to extract inline skill template from install.sh"
    rm -f "$expected_file" "$inline_file" "$diff_file"
    return 0
  fi

  if ! diff -u "$expected_file" "$inline_file" > "$diff_file"; then
    fail "install.sh inline skill template is out of sync with bundled SKILL.md"
    warn "First lines of diff:"
    sed -n '1,80p' "$diff_file" >&2 || true
  fi

  rm -f "$expected_file" "$inline_file" "$diff_file"
}

main() {
  check_frontmatter
  check_marker_and_links
  check_key_paths_exist
  check_inline_sync

  if [ "$ERROR_COUNT" -gt 0 ]; then
    fail "skill-smoke detected ${ERROR_COUNT} issue(s)"
    exit 1
  fi

  ok "skill-smoke checks passed"
}

main "$@"
