#!/bin/bash
# TDD-06-2: manifest.json should list the agent
set -e
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  pi --max-turns 1 --no-input -p \
  -e "$EXT" "hi" 2>&1 >/dev/null || true

MANIFEST="$TEST_WS/manifest.json"
if [ -f "$MANIFEST" ]; then
  if jq -e '.agents.test1' "$MANIFEST" >/dev/null 2>&1; then
    echo "PASS: manifest.json contains agent 'test1'"
    rm -rf "$TEST_WS" 2>/dev/null || true
    exit 0
  else
    echo "FAIL: manifest.json doesn't contain agent"
    cat "$MANIFEST"
    rm -rf "$TEST_WS" 2>/dev/null || true
    exit 1
  fi
else
  echo "FAIL: manifest.json does not exist"
  rm -rf "$TEST_WS" 2>/dev/null || true
  exit 1
fi
