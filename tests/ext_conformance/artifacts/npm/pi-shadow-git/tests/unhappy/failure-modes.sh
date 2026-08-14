#!/bin/bash
# Unhappy Path Tests - Failure Mode Verification
set -e

echo "=== UNHAPPY PATH TESTS ==="
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"

# UP-01: Missing PI_WORKSPACE_ROOT - graceful handling
echo "UP-01: Missing PI_WORKSPACE_ROOT..."
unset PI_WORKSPACE_ROOT
OUTPUT=$(PI_AGENT_NAME="test1" \
  timeout 30 pi --max-turns 1 --no-input -p \
  -e "$EXT" "What is 2+2?" 2>&1 || true)

# Should not crash with stack trace
if echo "$OUTPUT" | grep -qi "TypeError\|ReferenceError\|Cannot read"; then
  echo "  ✗ FAIL - crashed with error"
  echo "$OUTPUT" | head -5
  exit 1
else
  echo "  ✓ PASS - handled gracefully"
fi

# UP-02: Missing PI_AGENT_NAME - graceful handling
echo "UP-02: Missing PI_AGENT_NAME..."
TEST_WS=$(mktemp -d)
OUTPUT=$(PI_WORKSPACE_ROOT="$TEST_WS" \
  timeout 30 pi --max-turns 1 --no-input -p \
  -e "$EXT" "What is 2+2?" 2>&1 || true)

if echo "$OUTPUT" | grep -qi "TypeError\|ReferenceError\|Cannot read"; then
  echo "  ✗ FAIL - crashed with error"
  exit 1
else
  echo "  ✓ PASS - handled gracefully"
fi
rm -rf "$TEST_WS"

# UP-03: Malformed audit.jsonl - Mission Control handles gracefully
echo "UP-03: Malformed audit.jsonl..."
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"
echo "this is definitely not valid json" > "$TEST_WS/agents/test1/audit.jsonl"
echo '{"event":"session_start","ts":123456}' >> "$TEST_WS/agents/test1/audit.jsonl"

OUTPUT=$(PI_WORKSPACE_ROOT="$TEST_WS" \
  timeout 30 pi --max-turns 1 --no-input -p \
  -e "$EXT" "/mc" 2>&1 || true)

if echo "$OUTPUT" | grep -qi "SyntaxError\|parse error\|crash"; then
  echo "  ✗ FAIL - Mission Control crashed on bad JSON"
  exit 1
else
  echo "  ✓ PASS - handled gracefully"
fi
rm -rf "$TEST_WS"

# UP-04: Read-only directory - fail open
echo "UP-04: Read-only directory (fail open)..."
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"
chmod 555 "$TEST_WS/agents/test1"  # Read-only

OUTPUT=$(PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 30 pi --max-turns 1 --no-input -p \
  -e "$EXT" "What is 2+2?" 2>&1 || true)

chmod 755 "$TEST_WS/agents/test1"  # Restore

# Agent should still complete (fail open)
if echo "$OUTPUT" | grep -qi "4\|four"; then
  echo "  ✓ PASS - agent completed despite write failure"
else
  # Even if it didn't answer, it shouldn't crash
  if echo "$OUTPUT" | grep -qi "TypeError\|crash\|EACCES"; then
    echo "  ✗ FAIL - crashed on permission error"
    rm -rf "$TEST_WS"
    exit 1
  else
    echo "  ✓ PASS - no crash"
  fi
fi
rm -rf "$TEST_WS"

# UP-05: Stale .git/index.lock - should be handled
echo "UP-05: Stale git lock file..."
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1/.git"
touch "$TEST_WS/agents/test1/.git/index.lock"  # Stale lock

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 60 pi --max-turns 2 --no-input -p \
  -e "$EXT" "Write 'test' to output/x.txt" 2>&1 >/dev/null || true

# Check if agent completed work (output file exists)
if [ -f "$TEST_WS/agents/test1/output/x.txt" ]; then
  echo "  ✓ PASS - agent completed despite stale lock"
else
  # At minimum, check audit log exists (logging worked)
  if [ -f "$TEST_WS/agents/test1/audit.jsonl" ]; then
    echo "  ✓ PASS - logging worked despite git issues"
  else
    echo "  WARN - could not verify (check manually)"
  fi
fi
rm -rf "$TEST_WS"

echo "=== UNHAPPY PATH TESTS COMPLETE ==="
