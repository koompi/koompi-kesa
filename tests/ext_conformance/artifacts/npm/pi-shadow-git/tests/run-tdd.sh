#!/bin/bash
# TDD Cycle Runner
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
export EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"

if [ -z "$1" ]; then
  echo "═══════════════════════════════════════════════"
  echo "  TDD CYCLE RUNNER"
  echo "═══════════════════════════════════════════════"
  echo ""
  echo "Usage:"
  echo "  ./tests/run-tdd.sh <cycle>      Run specific cycle"
  echo "  ./tests/run-tdd.sh all          Run all TDD tests"
  echo "  ./tests/run-tdd.sh red          Run all, expect ALL to FAIL"
  echo ""
  echo "Available cycles:"
  for f in "$SCRIPT_DIR"/tdd/tdd-*.sh; do
    name=$(basename "$f" .sh | sed 's/-[a-z-]*$//')
    desc=$(head -2 "$f" | tail -1 | sed 's/^# //')
    echo "  $(basename "$f" .sh)"
  done
  exit 0
fi

if [ "$1" = "all" ]; then
  echo "═══════════════════════════════════════════════"
  echo "  RUNNING ALL TDD TESTS"
  echo "═══════════════════════════════════════════════"
  PASSED=0
  FAILED=0
  for test in "$SCRIPT_DIR"/tdd/tdd-*.sh; do
    name=$(basename "$test" .sh)
    echo ""
    echo "── $name ──"
    if bash "$test"; then
      ((PASSED++))
    else
      ((FAILED++))
    fi
  done
  echo ""
  echo "═══════════════════════════════════════════════"
  echo "  PASSED: $PASSED  FAILED: $FAILED"
  echo "═══════════════════════════════════════════════"
  [ "$FAILED" -eq 0 ] && exit 0 || exit 1
fi

if [ "$1" = "red" ]; then
  echo "═══════════════════════════════════════════════"
  echo "  RED PHASE: All tests should FAIL"
  echo "═══════════════════════════════════════════════"
  ALL_RED=1
  for test in "$SCRIPT_DIR"/tdd/tdd-*.sh; do
    name=$(basename "$test" .sh)
    echo -n "$name: "
    if bash "$test" >/dev/null 2>&1; then
      echo "PASS ✗ (should have failed!)"
      ALL_RED=0
    else
      echo "FAIL ✓ (correct - RED)"
    fi
  done
  echo ""
  if [ "$ALL_RED" -eq 1 ]; then
    echo "✅ All tests are RED - ready to implement"
    exit 0
  else
    echo "⚠️  Some tests already pass - verify they test new behavior"
    exit 1
  fi
fi

# Single cycle
TEST_FILE="$SCRIPT_DIR/tdd/$1.sh"
if [ ! -f "$TEST_FILE" ]; then
  TEST_FILE=$(ls "$SCRIPT_DIR"/tdd/$1*.sh 2>/dev/null | head -1)
fi

if [ -z "$TEST_FILE" ] || [ ! -f "$TEST_FILE" ]; then
  echo "Test not found: $1"
  exit 1
fi

echo "═══════════════════════════════════════════════"
echo "  TDD CYCLE: $(basename "$TEST_FILE" .sh)"
echo "═══════════════════════════════════════════════"
bash "$TEST_FILE"
