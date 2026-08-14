#!/bin/bash
# Verify current functionality before refactoring
set -e
RESULTS="$(dirname "$0")/results/functional.log"
mkdir -p "$(dirname "$RESULTS")"

echo "=== FUNCTIONAL BASELINE: $(date -Iseconds) ===" | tee "$RESULTS"

EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"

# Test 1: audit.jsonl created
echo -n "TEST: audit.jsonl created... " | tee -a "$RESULTS"
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 30 pi --max-turns 1 --no-input -p \
  -e "$EXT" "Say hello" 2>&1 >/dev/null || true

if [ -f "$TEST_WS/agents/test1/audit.jsonl" ]; then
  echo "PASS" | tee -a "$RESULTS"
else
  echo "FAIL" | tee -a "$RESULTS"
  rm -rf "$TEST_WS"
  exit 1
fi
rm -rf "$TEST_WS"

# Test 2: session_start event
echo -n "TEST: session_start event logged... " | tee -a "$RESULTS"
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 30 pi --max-turns 1 --no-input -p \
  -e "$EXT" "Say hello" 2>&1 >/dev/null || true

if grep -q '"event":"session_start"' "$TEST_WS/agents/test1/audit.jsonl" 2>/dev/null; then
  echo "PASS" | tee -a "$RESULTS"
else
  echo "FAIL" | tee -a "$RESULTS"
  rm -rf "$TEST_WS"
  exit 1
fi
rm -rf "$TEST_WS"

# Test 3: killswitch works
echo -n "TEST: killswitch disables logging... " | tee -a "$RESULTS"
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 30 pi --max-turns 1 --no-input -p \
  -e "$EXT" "hi" 2>&1 >/dev/null || true
BEFORE=$(wc -l < "$TEST_WS/agents/test1/audit.jsonl" 2>/dev/null | tr -d ' ' || echo 0)

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" PI_SHADOW_GIT_DISABLED=1 \
  timeout 30 pi --max-turns 1 --no-input -p \
  -e "$EXT" "bye" 2>&1 >/dev/null || true
AFTER=$(wc -l < "$TEST_WS/agents/test1/audit.jsonl" 2>/dev/null | tr -d ' ' || echo 0)

if [ "$AFTER" -eq "$BEFORE" ]; then
  echo "PASS" | tee -a "$RESULTS"
else
  echo "FAIL (grew from $BEFORE to $AFTER)" | tee -a "$RESULTS"
  rm -rf "$TEST_WS"
  exit 1
fi
rm -rf "$TEST_WS"

echo ""
echo "=== ALL FUNCTIONAL TESTS PASS ===" | tee -a "$RESULTS"
