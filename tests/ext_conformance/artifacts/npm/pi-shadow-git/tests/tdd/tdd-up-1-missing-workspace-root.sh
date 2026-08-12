#!/bin/bash
# TDD-UP-1: Missing KODE_WORKSPACE_ROOT should not crash
# Behavior: Agent handles gracefully (no TypeError/ReferenceError)
set -e
EXT="${EXT:-$HOME/.kode/agent/extensions/shadow-git.ts}"

unset KODE_WORKSPACE_ROOT
OUTPUT=$(KODE_AGENT_NAME="test1" \
  pi --max-turns 1 --no-input -p \
  -e "$EXT" "What is 2+2?" 2>&1 || true)

# ASSERTION: No crash (no TypeError, ReferenceError, Cannot read)
if echo "$OUTPUT" | grep -qi "TypeError\|ReferenceError\|Cannot read properties"; then
  echo "FAIL: crashed with JS error"
  echo "$OUTPUT" | grep -i "error" | head -3
  exit 1
else
  echo "PASS: handled missing KODE_WORKSPACE_ROOT gracefully"
  exit 0
fi
