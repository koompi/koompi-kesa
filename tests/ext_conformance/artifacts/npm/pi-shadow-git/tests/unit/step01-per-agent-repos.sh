#!/bin/bash
# STEP-01 Unit Tests: Per-Agent Git Repos
set -e

echo "=== STEP-01 UNIT TESTS ==="
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"

# UT-01-01: Agent has its own .git directory
echo -n "UT-01-01: Agent has own .git directory... "
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 30 pi --max-turns 1 --no-input -p \
  -e "$EXT" "hi" 2>&1 >/dev/null || true

if [ -d "$TEST_WS/agents/test1/.git" ]; then
  echo "PASS"
else
  echo "FAIL - no .git in agents/test1/"
  rm -rf "$TEST_WS"
  exit 1
fi
rm -rf "$TEST_WS"

# UT-01-02: .gitignore excludes audit.jsonl
echo -n "UT-01-02: .gitignore excludes audit.jsonl... "
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 30 pi --max-turns 1 --no-input -p \
  -e "$EXT" "hi" 2>&1 >/dev/null || true

if [ -f "$TEST_WS/agents/test1/.gitignore" ] && grep -q "audit.jsonl" "$TEST_WS/agents/test1/.gitignore"; then
  echo "PASS"
else
  echo "FAIL - audit.jsonl not gitignored"
  rm -rf "$TEST_WS"
  exit 1
fi
rm -rf "$TEST_WS"

# UT-01-03: Workspace root .git is not modified by agents
echo -n "UT-01-03: Workspace root .git unchanged... "
TEST_WS=$(mktemp -d)
cd "$TEST_WS"
git init >/dev/null 2>&1
mkdir -p agents/test1
git add -A && git commit -m "init" >/dev/null 2>&1
ROOT_BEFORE=$(git rev-parse HEAD)

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 60 pi --max-turns 2 --no-input -p \
  -e "$EXT" "Write 'hello' to output/test.txt" 2>&1 >/dev/null || true

ROOT_AFTER=$(git rev-parse HEAD)
if [ "$ROOT_BEFORE" = "$ROOT_AFTER" ]; then
  echo "PASS"
else
  echo "FAIL - root git was modified"
  rm -rf "$TEST_WS"
  exit 1
fi
rm -rf "$TEST_WS"

echo "=== STEP-01 UNIT TESTS COMPLETE ==="
