#!/bin/bash
# TDD-04-1: commitQueue should not exist in source
set -e
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"

if grep -q "commitQueue" "$EXT"; then
  echo "FAIL: commitQueue still exists in source"
  grep -n "commitQueue" "$EXT" | head -3
  exit 1
else
  echo "PASS: no commitQueue in source"
  exit 0
fi
