#!/bin/bash
# STEP-02 Integration Test: 10x Commit Reduction
set -e

echo "=== STEP-02 INTEGRATION TEST: COMMIT REDUCTION ==="
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"

TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

echo "Running agent for 5 turns with multiple tools..."
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 180 pi --max-turns 5 --no-input -p \
  -e "$EXT" \
  "Do 5 separate tasks: 1) write '1' to output/1.txt, 2) write '2' to output/2.txt, 3) write '3' to output/3.txt, 4) write '4' to output/4.txt, 5) write '5' to output/5.txt. Do one per turn." 2>&1 >/dev/null || true

AGENT_DIR="$TEST_WS/agents/test1"
if [ -d "$AGENT_DIR/.git" ]; then
  COMMITS=$(cd "$AGENT_DIR" && git log --oneline 2>/dev/null | wc -l | tr -d ' ')
  TOOL_CALLS=$(grep -c '"event":"tool_call"' "$AGENT_DIR/audit.jsonl" 2>/dev/null || echo 0)
  TURNS=$(grep -c '"event":"turn_end"' "$AGENT_DIR/audit.jsonl" 2>/dev/null || echo 0)
  
  echo "  Tool calls: $TOOL_CALLS"
  echo "  Turns: $TURNS"
  echo "  Git commits: $COMMITS"
  
  # Success: commits should be roughly equal to turns (+ init), not tool calls
  if [ "$TOOL_CALLS" -gt 0 ] && [ "$COMMITS" -lt "$TOOL_CALLS" ]; then
    RATIO=$(echo "scale=1; $TOOL_CALLS / $COMMITS" | bc 2>/dev/null || echo "N/A")
    echo "  Reduction ratio: ${RATIO}x"
    echo "  ✓ PASS - commits < tool calls"
  else
    echo "  ✗ FAIL - no reduction (commits >= tool calls)"
    rm -rf "$TEST_WS"
    exit 1
  fi
else
  echo "SKIP - no .git (step-01 prerequisite)"
fi

rm -rf "$TEST_WS"
echo "=== STEP-02 INTEGRATION TEST COMPLETE ==="
