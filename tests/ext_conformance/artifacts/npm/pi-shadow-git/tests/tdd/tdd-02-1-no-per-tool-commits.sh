#!/bin/bash
# TDD-02-1: Commits should happen per-TURN, not per-TOOL
#
# GOAL: Verify we're NOT doing per-tool commits
#
# Per-tool commits (BAD): 1 commit for each tool call
#   - 10 tools = 10 tool commits
#
# Turn-level commits (GOOD): 1 commit for each turn
#   - 10 tools in 1 turn = 1 turn commit
#
# EXPECTED COMMITS:
#   - "agent initialized" (git init)
#   - "[agent:start] session began" (session start)
#   - "[agent:turn-N]" for each turn
#
# NOT EXPECTED:
#   - "[agent:tool]" commits (removed in this refactor)
#
set -e
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

# Run agent - should make multiple tool calls in 1 turn
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  pi --max-turns 2 --no-input -p \
  -e "$EXT" "Write 'a' to output/a.txt and 'b' to output/b.txt" 2>&1 >/dev/null || true

AGENT_DIR="$TEST_WS/agents/test1"

if [ ! -d "$AGENT_DIR/.git" ]; then
  echo "FAIL: no .git in agent directory"
  rm -rf "$TEST_WS" 2>/dev/null || true
  exit 1
fi

cd "$AGENT_DIR"

# Count different commit types
TOOL_COMMITS=$(git log --oneline 2>/dev/null | grep -c ":tool]" 2>/dev/null || echo "0")
TURN_COMMITS=$(git log --oneline 2>/dev/null | grep -c ":turn-" 2>/dev/null || echo "0")
TOTAL_COMMITS=$(git log --oneline 2>/dev/null | wc -l | tr -d ' \n')
TOOL_CALLS=$(grep -c '"event":"tool_call"' audit.jsonl 2>/dev/null || echo "0")

# Ensure numeric values (strip any whitespace)
TOOL_COMMITS=$(echo "$TOOL_COMMITS" | tr -d ' \n')
TURN_COMMITS=$(echo "$TURN_COMMITS" | tr -d ' \n')
TOOL_CALLS=$(echo "$TOOL_CALLS" | tr -d ' \n')

echo "Tool calls: ${TOOL_CALLS:-0}"
echo "Total commits: ${TOTAL_COMMITS:-0}"
echo "  - Tool commits: ${TOOL_COMMITS:-0} (should be 0)"
echo "  - Turn commits: ${TURN_COMMITS:-0}"

rm -rf "$TEST_WS" 2>/dev/null || true

# ASSERTION: Zero per-tool commits (we only do turn-level now)
if [ "$TOOL_COMMITS" -eq 0 ]; then
  echo "PASS: zero per-tool commits (turn-level only)"
  exit 0
else
  echo "FAIL: found $TOOL_COMMITS per-tool commits"
  exit 1
fi
