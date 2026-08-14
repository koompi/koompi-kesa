#!/bin/bash
# TDD-06-1: manifest.json should exist at workspace root
set -e
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  pi --max-turns 1 --no-input -p \
  -e "$EXT" "hi" 2>&1 >/dev/null || true

MANIFEST="$TEST_WS/manifest.json"
if [ -f "$MANIFEST" ]; then
  echo "PASS: manifest.json exists"
  rm -rf "$TEST_WS" 2>/dev/null || true
  exit 0
else
  echo "FAIL: manifest.json does not exist"
  rm -rf "$TEST_WS" 2>/dev/null || true
  exit 1
fi
