#!/bin/bash
# Regression tests - run after EVERY change
set -e

echo "=== REGRESSION TESTS ==="
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"

# RT-01: audit.jsonl created
echo -n "RT-01: audit.jsonl created... "
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 30 pi --max-turns 1 --no-input -p \
  -e "$EXT" "hi" 2>&1 >/dev/null || true
if [ -f "$TEST_WS/agents/test1/audit.jsonl" ]; then echo "PASS"; else echo "FAIL"; rm -rf "$TEST_WS"; exit 1; fi
rm -rf "$TEST_WS"

# RT-02: session_start logged
echo -n "RT-02: session_start logged... "
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 30 pi --max-turns 1 --no-input -p \
  -e "$EXT" "hi" 2>&1 >/dev/null || true
if grep -q '"event":"session_start"' "$TEST_WS/agents/test1/audit.jsonl" 2>/dev/null; then echo "PASS"; else echo "FAIL"; rm -rf "$TEST_WS"; exit 1; fi
rm -rf "$TEST_WS"

# RT-03: tool_call logged
echo -n "RT-03: tool_call logged... "
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 60 pi --max-turns 2 --no-input -p \
  -e "$EXT" "Write 'x' to output/x.txt" 2>&1 >/dev/null || true
if grep -q '"event":"tool_call"' "$TEST_WS/agents/test1/audit.jsonl" 2>/dev/null; then echo "PASS"; else echo "FAIL"; rm -rf "$TEST_WS"; exit 1; fi
rm -rf "$TEST_WS"

# RT-04: killswitch works
echo -n "RT-04: killswitch works... "
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
if [ "$AFTER" -eq "$BEFORE" ]; then echo "PASS"; else echo "FAIL"; rm -rf "$TEST_WS"; exit 1; fi
rm -rf "$TEST_WS"

echo "=== ALL REGRESSION TESTS PASS ==="
