#!/bin/bash
# Baseline metrics capture - run BEFORE any refactor work
set -e
BASELINE_DIR="$(dirname "$0")/results"
mkdir -p "$BASELINE_DIR"

echo "=== BASELINE CAPTURE: $(date -Iseconds) ===" | tee "$BASELINE_DIR/baseline.log"

EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"
if [ ! -f "$EXT" ]; then
  echo "ERROR: Extension not found at $EXT"
  exit 1
fi

# 1. Single agent commit count
echo "Test: Single agent, measure commits..."
TEST_WS=$(mktemp -d)
cd "$TEST_WS"
git init >/dev/null 2>&1
mkdir -p agents/test1/{workspace,output}
git add -A && git commit -m "init" >/dev/null 2>&1

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 60 pi --max-turns 3 --no-input -p \
  -e "$EXT" \
  "Write the number 1 to output/one.txt, then 2 to output/two.txt" 2>&1 >/dev/null || true

COMMIT_COUNT=$(git log --oneline 2>/dev/null | wc -l | tr -d ' ')
echo "single_agent_commits=$COMMIT_COUNT" >> "$BASELINE_DIR/metrics.txt"
echo "  Commits: $COMMIT_COUNT" | tee -a "$BASELINE_DIR/baseline.log"

# 2. Parallel agents - lock conflict rate
echo "Test: 3 parallel agents, measure lock conflicts..."
TEST_WS2=$(mktemp -d)
cd "$TEST_WS2"
git init >/dev/null 2>&1
mkdir -p agents/{a1,a2,a3}/{workspace,output}
git add -A && git commit -m "init" >/dev/null 2>&1

for agent in a1 a2 a3; do
  PI_WORKSPACE_ROOT="$TEST_WS2" PI_AGENT_NAME="$agent" \
    timeout 60 pi --max-turns 2 --no-input -p \
    -e "$EXT" \
    "Write 'hello from $agent' to output/greeting.txt" 2>&1 >/dev/null &
done
wait

LOCK_ERRORS=$(grep -r "index.lock\|lock conflict" "$TEST_WS2"/agents/*/audit.jsonl 2>/dev/null | wc -l | tr -d ' ' || echo 0)
echo "parallel_lock_errors=$LOCK_ERRORS" >> "$BASELINE_DIR/metrics.txt"
echo "  Lock errors: $LOCK_ERRORS" | tee -a "$BASELINE_DIR/baseline.log"

rm -rf "$TEST_WS" "$TEST_WS2"

echo ""
echo "=== BASELINE COMPLETE ===" | tee -a "$BASELINE_DIR/baseline.log"
cat "$BASELINE_DIR/metrics.txt"
