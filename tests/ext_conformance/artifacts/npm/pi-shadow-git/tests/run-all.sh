#!/bin/bash
# Master Test Runner for Shadow-Git Refactor
# Run this before and after EVERY change

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
export EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"
RESULTS_DIR="$SCRIPT_DIR/results"
mkdir -p "$RESULTS_DIR"

echo "════════════════════════════════════════════════════════════"
echo "  SHADOW-GIT TEST SUITE"
echo "════════════════════════════════════════════════════════════"
echo "  Extension: $EXT"
echo "  Started:   $(date -Iseconds)"
echo "════════════════════════════════════════════════════════════"
echo ""

# Verify extension exists
if [ ! -f "$EXT" ]; then
  echo "⛔ ERROR: Extension not found at $EXT"
  echo "   Set EXT environment variable to the correct path"
  exit 1
fi

# Counters
PASSED=0
FAILED=0
SKIPPED=0

run_test() {
  local name=$1
  local script=$2
  
  echo "┌── Running: $name"
  echo "│"
  
  set +e
  output=$(bash "$script" 2>&1)
  status=$?
  set -e
  
  echo "$output" | sed 's/^/│   /'
  echo "$output" > "$RESULTS_DIR/$name.log"
  
  if [ $status -eq 0 ]; then
    echo "│"
    echo "└── ✓ $name PASSED"
    ((PASSED++))
  else
    echo "│"
    echo "└── ✗ $name FAILED"
    ((FAILED++))
  fi
  echo ""
}

# ═══════════════════════════════════════════════════════════════
# PHASE 0: BASELINE (Must pass before ANY implementation)
# ═══════════════════════════════════════════════════════════════
echo "▶ PHASE 0: BASELINE VERIFICATION"
echo "─────────────────────────────────"
run_test "baseline-functional" "$SCRIPT_DIR/baseline/verify-functionality.sh"

if [ "$FAILED" -gt 0 ]; then
  echo ""
  echo "⛔ BASELINE FAILED - STOP!"
  echo "   Do not proceed with implementation until baseline passes."
  exit 1
fi
echo ""

# ═══════════════════════════════════════════════════════════════
# REGRESSION TESTS (Run after every change)
# ═══════════════════════════════════════════════════════════════
echo "▶ REGRESSION TESTS"
echo "──────────────────"
run_test "regression-core" "$SCRIPT_DIR/regression/core-functionality.sh"

if [ "$FAILED" -gt 0 ]; then
  echo ""
  echo "⛔ REGRESSION FAILURE - REVERT!"
  echo "   Something broke. Revert your changes and investigate."
  exit 1
fi
echo ""

# ═══════════════════════════════════════════════════════════════
# UNIT TESTS (Step-specific)
# ═══════════════════════════════════════════════════════════════
echo "▶ UNIT TESTS"
echo "────────────"
for test_script in "$SCRIPT_DIR"/unit/step*.sh; do
  if [ -f "$test_script" ]; then
    name=$(basename "$test_script" .sh)
    run_test "unit-$name" "$test_script"
  fi
done
echo ""

# ═══════════════════════════════════════════════════════════════
# INTEGRATION TESTS
# ═══════════════════════════════════════════════════════════════
echo "▶ INTEGRATION TESTS"
echo "───────────────────"
for test_script in "$SCRIPT_DIR"/integration/step*.sh; do
  if [ -f "$test_script" ]; then
    name=$(basename "$test_script" .sh)
    run_test "integration-$name" "$test_script"
  fi
done
echo ""

# ═══════════════════════════════════════════════════════════════
# HOT PATH TESTS
# ═══════════════════════════════════════════════════════════════
echo "▶ HOT PATH TESTS"
echo "────────────────"
if [ -f "$SCRIPT_DIR/hotpath/hot-paths.sh" ]; then
  run_test "hot-paths" "$SCRIPT_DIR/hotpath/hot-paths.sh"
fi
echo ""

# ═══════════════════════════════════════════════════════════════
# UNHAPPY PATH TESTS
# ═══════════════════════════════════════════════════════════════
echo "▶ UNHAPPY PATH TESTS"
echo "────────────────────"
if [ -f "$SCRIPT_DIR/unhappy/failure-modes.sh" ]; then
  run_test "unhappy-paths" "$SCRIPT_DIR/unhappy/failure-modes.sh"
fi
echo ""

# ═══════════════════════════════════════════════════════════════
# SUMMARY
# ═══════════════════════════════════════════════════════════════
echo "════════════════════════════════════════════════════════════"
echo "  TEST SUMMARY"
echo "════════════════════════════════════════════════════════════"
echo "  Passed:  $PASSED"
echo "  Failed:  $FAILED"
echo "  Skipped: $SKIPPED"
echo "  Completed: $(date -Iseconds)"
echo "════════════════════════════════════════════════════════════"
echo ""

if [ "$FAILED" -gt 0 ]; then
  echo "⛔ $FAILED TEST(S) FAILED"
  echo ""
  echo "BACKPRESSURE PROTOCOL:"
  echo "  1. Do NOT proceed to next step"
  echo "  2. Investigate failed tests in $RESULTS_DIR/"
  echo "  3. Fix issues or revert changes"
  echo "  4. Re-run tests until all pass"
  exit 1
else
  echo "✅ ALL TESTS PASSED"
  echo ""
  echo "PROCEED CONDITIONS MET:"
  echo "  ✓ Baseline verified"
  echo "  ✓ Regression tests pass"
  echo "  ✓ Unit tests pass"
  echo "  ✓ Integration tests pass"
  echo "  ✓ Hot paths verified"
  echo "  ✓ Failure modes handled"
  echo ""
  echo "You may proceed to the next implementation step."
  exit 0
fi
